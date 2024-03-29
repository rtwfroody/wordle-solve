use clap::Parser;
use dirs;
use hex;
use indicatif::{ParallelProgressIterator, ProgressStyle};
use rayon::prelude::*;
use serde_json;
use sha2::{Sha256, Digest};
use std::{cmp, fs};
use std::collections::{HashSet, HashMap};
use std::fs::File;
use std::io::{self, BufRead};
use std::str::Chars;
use std::sync::Mutex;

#[derive(Parser)]
/// Print out the next best (hopefully) guess when solving a wordle puzzle.
///
/// Each constraint describes a single wordle result row. Put a - in front of
/// each character that is gray, a ~ in front of each character that is yellow,
/// and leave the green ones as is.
///
/// Example: wordle-solve -- "-r -a ~i -s -e" "-h -o ~t -l y"
#[command(author, version, about)]
struct Cli {
    #[arg(short, long, value_name = "FILE")]
    words: Option<String>,
    /// One or more wordle result rows.
    constraint: Vec<String>,
    /// See how the algorithm performs against the given word.
    #[arg(short, long, value_name = "TEST")]
    test: Option<String>,
    /// See how the algorithm performs against every word.
    #[arg(long)]
    full_test: bool
}

#[derive(Clone)]
#[derive(Debug)]
struct CharacterConstraint {
    is: Option<char>,
    is_not: HashSet<char>
}

#[derive(Debug)]
struct Constraint {
    character: Vec<CharacterConstraint>,
    // For each char, track how many there are at least in the word.
    min_occurrence: HashMap<char, usize>,
    max_occurrence: HashMap<char, usize>
}

impl Constraint {
    pub fn new(size: usize) -> Self {
        let character = vec![CharacterConstraint {is: None, is_not: HashSet::new() }; size];
        Self {
            character,
            min_occurrence: HashMap::new(),
            max_occurrence: HashMap::new()
        }
    }

    pub fn from_string(string: &String, size: usize) -> Self {
        let mut constraint = Constraint::new(size);
        let mut i = 0;
        enum Op {
            Green,
            Yellow,
            Gray
        }
        let mut op = Op::Green;
        let mut count: HashMap<char, usize> = HashMap::new();
        let mut found_max = HashSet::new();
        for c in string.chars() {
            match c {
                ' ' => { i += 1; op = Op::Green; },
                '-' => op = Op::Gray,
                '~' => op = Op::Yellow,
                x => {
                    match op {
                        Op::Green => {
                            constraint.character[i].is = Some(x);
                            constraint.increment_min_occurrence(&x);
                            count.entry(x).and_modify(|x| *x += 1).or_insert(1);
                        },
                        Op::Yellow => {
                            constraint.character[i].is_not.insert(x);
                            constraint.increment_min_occurrence(&x);
                            count.entry(x).and_modify(|x| *x += 1).or_insert(1);
                        },
                        Op::Gray => {
                            constraint.character[i].is_not.insert(x);
                            found_max.insert(x);
                            ()
                        }
                    }
                }
            }
        }
        for c in found_max {
            constraint.max_occurrence.insert(c, *count.get(&c).unwrap_or(&0));
        }
        constraint
    }

    pub fn increment_min_occurrence(&mut self, c: &char) {
        self.min_occurrence.entry(*c).and_modify(|n| *n += 1).or_insert(1);
    }

    pub fn update(&mut self, constraint: &Constraint) {
        for (c, count) in constraint.min_occurrence.iter() {
            self.min_occurrence.entry(*c)
                    .and_modify(|v| *v = cmp::max(*v, *count))
                    .or_insert(*count);
        }
        for (c, count) in constraint.max_occurrence.iter() {
            self.max_occurrence.entry(*c)
                    .and_modify(|v| *v = cmp::min(*v, *count))
                    .or_insert(*count);
        }
        for (my_c, other_c) in self.character.iter_mut().zip(constraint.character.iter()) {
            if other_c.is == None {
                for c in other_c.is_not.iter() {
                    my_c.is_not.insert(*c);
                }
            } else {
                my_c.is = other_c.is;
            }
        }
    }

    pub fn allows(&self, word: &Word) -> bool
    {
        self.min_occurrence.iter()
                .all(|(key, value)| word.char_count(key) >= *value) &&
        self.max_occurrence.iter()
                .all(|(key, value)| word.char_count(key) <= *value) &&
        // Check that green letters are where they should be.
        self.character.iter().zip(word.chars())
                .all(|(cc, y)|
                        match cc.is {
                            None => true,
                            Some(x) => x == y
                        }) &&
        self.character.iter().zip(word.chars())
                .all(|(cc, y)| !cc.is_not.contains(&y))
    }
}

#[derive(Clone)]
#[derive(PartialEq, Eq)]
struct Word {
    word: String,
    char_frequency: HashMap<char, usize>
}

impl Ord for Word{
    fn cmp(&self, other: &Self) -> cmp::Ordering {
        self.word.cmp(&other.word)
    }
}

impl PartialOrd for Word{
    fn partial_cmp(&self, other: &Self) -> Option<cmp::Ordering> {
        self.word.partial_cmp(&other.word)
    }
}

impl Word {
    pub fn new(word: String) -> Self
    {
        let char_frequency = char_frequency(word.chars());
        Self { word, char_frequency }
    }

    pub fn char_count(&self, c: &char) -> usize
    {
        *self.char_frequency.get(c).unwrap_or(&0)
    }

    pub fn chars(&self) -> Chars<'_>
    {
        self.word.chars()
    }

    pub fn len(&self) -> usize
    {
        self.word.len()
    }
}

fn char_frequency(chars: Chars) -> HashMap<char, usize>
{
    let mut char_frequency = HashMap::new();
    for c in chars {
        char_frequency.entry(c).and_modify(|n| *n += 1).or_insert(1);
    }
    char_frequency
}

fn wordle_guess(guess: &Word, answer: &Word) -> Constraint
{
    let mut constraint: Constraint = Constraint::new(guess.len());
    for (i, (g, a)) in guess.chars().zip(answer.chars()).enumerate() {
        if g == a {
            constraint.character[i].is = Some(g);
        } else {
            constraint.character[i].is_not.insert(g);
        }
    }

    let guess_frequency = char_frequency(guess.chars());

    for (c, guess_count) in guess_frequency.iter() {
        let answer_count = answer.char_count(c);
        let min_count = cmp::min(*guess_count, answer_count);
        if min_count > 0 {
            constraint.min_occurrence.insert(*c, min_count);
        }
        if *guess_count > answer_count {
            constraint.max_occurrence.insert(*c, answer_count);
        }
    }

    constraint
}

fn filter_words<'a>(constraint: &Constraint, words: &'a Vec<Word>) -> Vec<&'a Word>
{
    let mut v = Vec::new();

    for word in words {
        if constraint.allows(word) {
            v.push(word);
        }
    }

    v
}

fn score_guess_count_eliminations(guess: &Word, words: &Vec<&Word>, constraint: &Constraint) -> usize
{
    let mut score = words.len() * words.len();
    for answer in words {
        // If the word is `word`, then how good is this guess?
        let mut answer_constraint = wordle_guess(guess, answer);
        answer_constraint.update(&constraint);
        score -= words.iter().filter(|w| answer_constraint.allows(w)).count();
    }
    score
}

fn read_words(path: &String) -> Result<(Vec<Word>, String), String>
{
    let mut words = Vec::new();
    let mut hasher = Sha256::new();

    let file = match File::open(path) {
        Ok(file) => file,
        Err(error) => panic!("Failed to open {}: {:?}", path, error)
    };

    let mut word_length = None;
    for line_result in io::BufReader::new(file).lines() {
        let line = line_result.unwrap();
        let l1 = line.chars().count();
        match word_length {
            Some(l2) => if l1 != l2 {
                return Err(format!("Some lines in {} contain {} characters while others contain {} characters (e.g. {}).",
                    path, l1, l2, line));
            },
            None => word_length = Some(l1)
        }
        hasher.update(&line);
        words.push(Word::new(line));
    }
    Ok((words, hex::encode(hasher.finalize())))
}

struct WordleSolver {
    words: Vec<Word>,
    first_guess: Mutex<Option<usize>>
}

impl WordleSolver {
    fn best_guess<'a>(&'a self, constraint: &Constraint, verbose: bool) ->
            Result<&'a Word, String>
    {
        let remaining_words = filter_words(constraint, &self.words);

        if remaining_words.len() == self.words.len() {
            let first_guess = self.first_guess.lock().unwrap();
            if let Some(index) = *first_guess {
                return Ok(&self.words[index]);
            }
        }

        if remaining_words.len() < 1 {
            return Err("Error: No words match those constraints.".to_string());
        }
        if remaining_words.len() == 1 {
            return Ok(remaining_words.first().unwrap());
        }
        if verbose {
            println!("{}/{} words remaining", remaining_words.len(), self.words.len());
            if remaining_words.len() < 15 {
                for w in &remaining_words {
                    println!("  {}", w.word)
                }
            }
        }

        if remaining_words.len() == 2 {
            return Ok(remaining_words.first().unwrap());
        }

        let style = ProgressStyle::with_template("{bar:60} {pos}/{len} {eta}").unwrap();

        let (_best_score, best_guess, index) =
            self.words
                    .par_iter()
                    .progress_with_style(style)
                    .map(|guess| (score_guess_count_eliminations(guess, &remaining_words, constraint), guess))
                    // Prefer words that might be the answer.
                    .map(|(score, guess)|
                        (score + if constraint.allows(guess) { 1 } else { 0 }, guess))
                    .enumerate()
                    .map(|(index, (score, guess))| (score, guess, index))
                    .max()
                    .unwrap();

        if remaining_words.len() == self.words.len() {
            let mut first_guess = self.first_guess.lock().unwrap();
            *first_guess = Some(index);
        }

        Ok(best_guess)
    }

    fn test<'a>(&'a self, answer: &Word, verbose: bool) -> Vec<&'a Word>
    {
        let mut result = Vec::new();
        let word_length = self.words.first().unwrap().len();
        let mut constraint = Constraint::new(word_length);
        for _ in 1..100 {
            let guess = self.best_guess(&constraint, false).unwrap();
            result.push(guess);
            if verbose {
                println!("Guess: {}", guess.word);
            }
            if guess.word.eq(&answer.word) {
                return result;
            }
            let guess_constraint = wordle_guess(&guess, answer);
            constraint.update(&guess_constraint);
        }
        return result;
    }

    fn full_test(&self)
    {
        let mut result = HashMap::new();
        for word in &self.words {
            let guesses = self.test(&word, false);
            print!("Guessed {} from", word.word);
            let count = guesses.len();
            result.entry(count).and_modify(|c| *c += 1).or_insert(1);
            for guess in guesses {
                print!(" {}", guess.word);
            }
            println!();
        }

        println!("{:?}", result);
    }
}

/// Return how many guesses it took to find the word.
fn main()
{
    let cli = Cli::parse();
    let mut cache_path = dirs::cache_dir().unwrap();
    cache_path.push("wordle-solve.cache");
    let cache_string = fs::read_to_string(&cache_path).unwrap_or_default();
    let mut cache : HashMap<String, usize> = serde_json::from_str(cache_string.as_str()).unwrap_or_default();

    let (words, hash) = read_words(&cli.words.unwrap_or("words".to_string())).unwrap();
    let word_length = words.first().unwrap().len();
    let solver = WordleSolver {
        words,
        first_guess: Mutex::new(cache.get(&hash).copied())
    };

    if cli.test.is_some() {
        let answer = Word::new(cli.test.unwrap());
        solver.test(&answer, true);

    } else if cli.full_test {
        solver.full_test();
        return;
    } else {
        let mut constraint_acc = Constraint::new(word_length);
        for constraint_string in cli.constraint {
            let constraint = Constraint::from_string(&constraint_string, word_length);
            constraint_acc.update(&constraint);
        }

        let guess = solver.best_guess(&constraint_acc, true).unwrap();

        println!("Best guess: {}", guess.word);
    }

    let first_guess = *solver.first_guess.lock().unwrap();
    if first_guess.is_some() {
        cache.insert(hash, first_guess.unwrap());
    }
    let cache_data = serde_json::to_string(&cache).unwrap();
    fs::write(&cache_path, &cache_data).unwrap();
}
