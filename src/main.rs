use std::fs::File;
use std::io::{self, BufRead};
use std::collections::{HashSet, HashMap};
use clap::Parser;
use std::cmp;

#[derive(Parser)]
#[command(author, version, about)]
struct Cli {
    #[arg(short, long, value_name = "FILE")]
    words: Option<String>,
    information: Vec<String>
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
    min_occurrence: HashMap<char, usize>
}

impl Constraint {
    pub fn new(size: usize) -> Self {
        let character = vec![CharacterConstraint {is: None, is_not: HashSet::new() }; size];
        Self {
            character,
            min_occurrence: HashMap::new()
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
        for c in string.chars() {
            match c {
                ' ' => { i += 1; op = Op::Green; },
                '-' => op = Op::Gray,
                '~' => op = Op::Yellow,
                x => {
                    match op {
                        Op::Green => {
                            constraint.character[i].is = Some(x);
                            constraint.increment_min_occurrence(&x)
                        },
                        Op::Yellow => {
                            constraint.character[i].is_not.insert(x);
                            constraint.increment_min_occurrence(&x)
                        },
                        Op::Gray => {
                            constraint.character[i].is_not.insert(x);
                            ()
                        }
                    }
                }
            }
        }
        constraint
    }

    pub fn score(self) -> usize {
        let mut s = self.min_occurrence.values().sum();
        for cc in self.character {
            s += if cc.is != None { 10 } else { 0 };
            s += cc.is_not.len();
        }
        s
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
}

fn char_frequency(word: &str) -> HashMap<char, usize>
{
    let mut char_frequency = HashMap::new();
    for c in word.chars() {
        char_frequency.entry(c).and_modify(|n| *n += 1).or_insert(1);
    }
    char_frequency
}

fn wordle_guess(guess: &str, answer: &str) -> Constraint
{
    let mut constraint: Constraint = Constraint::new(guess.len());
    for (i, (g, a)) in guess.chars().zip(answer.chars()).enumerate() {
        if g == a {
            constraint.character[i].is = Some(g);
        } else {
            constraint.character[i].is_not.insert(g);
            // TODO: increment_min_occurrence() if this character exists elsewhere in the word
        }
    }

    let guess_frequency = char_frequency(guess);
    let answer_frequency = char_frequency(answer);

    for (c, guess_count) in guess_frequency.iter() {
        let answer_count = answer_frequency.get(c).unwrap_or(&0);
        let constraint_count = cmp::min(guess_count, answer_count);
        if constraint_count > &0 {
            constraint.min_occurrence.insert(*c, *constraint_count);
        }
    }

    constraint
}

fn filter_words(constraint: Constraint, words: &Vec<String>) -> Vec<&String>
{
    let mut v = Vec::new();

    for word in words {
        let char_frequency = char_frequency(word.as_str());
        if
            constraint.min_occurrence.iter()
                    .all(|(key, value)| char_frequency.get(key).unwrap_or(&0) >= value) &&
            // Check that green letters are where they should be.
            constraint.character.iter().zip(word.chars())
                    .all(|(cc, y)| 
                            match cc.is {
                                None => true,
                                Some(x) => x == y
                            }) &&
            constraint.character.iter().zip(word.chars())
                    .all(|(cc, y)| !cc.is_not.contains(&y))
        {
            v.push(word);
        }
    }

    v
}

fn score_guess(guess: &str, words: &Vec<&String>) -> usize
{
    let mut score = 0;
    for answer in words {
        // If the word is `word`, then how good is this guess?
        let constraint = wordle_guess(guess, answer);
        score += constraint.score();
    }
    score
}

fn read_words(path: &String, word_length: usize) -> Vec<String>
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
        words.push(line);
    }
    words
}

fn main()
{
    let cli = Cli::parse();

    let word_length = 5;

    let words = read_words(&cli.words.unwrap_or("words".to_string()), word_length);

    let mut constraint_acc = Constraint::new(word_length);
    for constraint_string in cli.information {
        let constraint = Constraint::from_string(&constraint_string, word_length);
        println!("Constraint: {:?}", constraint);
        constraint_acc.update(&constraint);
    }

    let remaining_words = filter_words(constraint_acc, &words);
    println!("{}/{} words remaining", remaining_words.len(), words.len());
    if remaining_words.len() < 15 {
        for w in &remaining_words {
            println!("  {}", w)
        }
    }

    let mut best_score = 0;
    let mut best_guess : &String = words.first().unwrap();
    for guess in &words {
        let score = score_guess(guess.as_str(), &remaining_words);
        if score > best_score {
            best_score = score;
            best_guess = guess;
        }
    }
    println!("Best guess: {}", best_guess);
}
