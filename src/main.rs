use std::fs::File;
use std::io::{self, BufRead};
use std::collections::HashSet;
use clap::Parser;

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
    does_not_contain: HashSet<char>
}

impl Constraint {
    pub fn new(size: usize) -> Self {
        let character = vec![CharacterConstraint {is: None, is_not: HashSet::new() }; size];
        Self { character, does_not_contain: HashSet::new() }
    }

    pub fn score(self) -> usize {
        let mut s = self.does_not_contain.len();
        for c in self.character {
            s += if c.is != None { 10 } else { 0 };
            s += c.is_not.len();
        }
        s
    }
}

fn wordle_guess(guess: &str, answer: &str) -> Constraint
{
    let mut constraint: Constraint = Constraint::new(guess.len());
    for (i, (g, a)) in guess.chars().zip(answer.chars()).enumerate() {
        if g == a {
            constraint.character[i].is = Some(g);
        } else if answer.contains(g) {
            constraint.character[i].is_not.insert(g);
        } else {
            constraint.does_not_contain.insert(g);
        }
    }
    constraint
}

fn filter_words(constraint: Constraint, words: &Vec<String>) -> Vec<&String>
{
    let mut v = Vec::new();

    for word in words {
        if
            // Word must not contain letters that aren't present.
            !constraint.does_not_contain.iter().any(|&x| word.contains(x)) &&
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

    let mut constraint = Constraint::new(word_length);
    for constraint_string in cli.information {
        let mut i = 0;
        enum Op {
            Is,
            IsNot,
            Exclude
        }
        let mut op = Op::Is;
        for c in constraint_string.chars() {
            match c {
                ' ' => { i += 1; op = Op::Is; },
                '-' => op = Op::Exclude,
                '~' => op = Op::IsNot,
                x => {
                    match op {
                        Op::Is => constraint.character[i].is = Some(x),
                        Op::IsNot => {constraint.character[i].is_not.insert(x); ()},
                        Op::Exclude => {constraint.does_not_contain.insert(x); ()}
                    }
                }
            }
        }
        println!("Constraint: {:?}", constraint);
    }

    let remaining_words = filter_words(constraint, &words);
    println!("{}/{} words remaining", remaining_words.len(), words.len());
    if remaining_words.len() < 10 {
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
