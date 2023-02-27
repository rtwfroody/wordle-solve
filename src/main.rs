use clap::Parser;
use indicatif::{ParallelProgressIterator, ProgressStyle};
use rayon::prelude::*;
use std::cmp;
use std::collections::{HashSet, HashMap};
use std::fs::File;
use std::io::{self, BufRead};
use std::str::Chars;

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

    /*
    pub fn score(self) -> usize {
        let mut s = self.min_occurrence.values().sum();
        s += self.max_occurrence.len();
        for cc in self.character {
            s += if cc.is != None { 2 } else { 0 };
            s += cc.is_not.len();
        }
        s
    }
    */

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

/*
fn score_guess_heuristic(guess: &Word, words: &Vec<&Word>, constraint: &Constraint) -> usize
{
    let mut score = 0;
    for answer in words {
        // If the word is `word`, then how good is this guess?
        let mut answer_constraint = wordle_guess(guess, answer);
        answer_constraint.update(&constraint);
        score += answer_constraint.score();
    }
    score
}
*/

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

fn read_words(path: &String, word_length: usize) -> Vec<Word>
{
    let mut words = Vec::new();

    let file = match File::open(path) {
        Ok(file) => file,
        Err(error) => panic!("Failed to open {}: {:?}", path, error)
    };

    for line_result in io::BufReader::new(file).lines() {
        let line = line_result.unwrap();
        if line.chars().count() != word_length {
            continue;
        }
        words.push(Word::new(line));
    }
    words
}

struct Cache {
    first_guess: Option<Word>
}
static mut CACHE : Cache = Cache { first_guess: None };

fn best_guess<'a>(words: &'a Vec<Word>, constraint: &Constraint, verbose: bool) ->
        Result<&'a Word, String>
{
    let remaining_words = filter_words(constraint, &words);

    if remaining_words.len() == words.len() {
        unsafe {
            match &CACHE.first_guess {
                Some(w) => return Ok(w),
                None => ()
            }
        }
    }

    if remaining_words.len() < 1 {
        return Err("Error: No words match those constraints.".to_string());
    }
    if remaining_words.len() == 1 {
        return Ok(remaining_words.first().unwrap());
    }
    if verbose {
        println!("{}/{} words remaining", remaining_words.len(), words.len());
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

    let (_best_score, best_guess) =
        words
                .par_iter()
                .progress_with_style(style)
                .map(|guess| (score_guess_count_eliminations(guess, &remaining_words, constraint), guess))
                .max()
                .unwrap();

    if remaining_words.len() == words.len() {
        unsafe {
            CACHE.first_guess = Some(Word::new(best_guess.word.clone()));
        }
    }

    Ok(best_guess)
}

/// Return how many guesses it took to find the word.
fn test<'a>(words: &'a Vec<Word>, answer: &Word, verbose: bool) -> Vec<&'a Word>
{
    let mut result = Vec::new();
    let word_length = words.first().unwrap().len();
    let mut constraint = Constraint::new(word_length);
    for _ in 1..100 {
        let guess = best_guess(&words, &constraint, false).unwrap();
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

fn full_test(words: &Vec<Word>)
{
    let mut result = HashMap::new();
    for word in words {
        let guesses = test(words, &word, false);
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

fn main()
{
    let cli = Cli::parse();

    let word_length = 5;
    let words = read_words(&cli.words.unwrap_or("words".to_string()), word_length);

    if cli.test.is_some() {
        let answer = Word::new(cli.test.unwrap());
        test(&words, &answer, true);
        return;
    }

    if cli.full_test {
        full_test(&words);
        return;
    }

    let mut constraint_acc = Constraint::new(word_length);
    for constraint_string in cli.constraint {
        let constraint = Constraint::from_string(&constraint_string, word_length);
        //println!("Constraint: {:?}", constraint);
        constraint_acc.update(&constraint);
    }

    let guess = best_guess(&words, &constraint_acc, true).unwrap();

    println!("Best guess: {}", guess.word);
}
