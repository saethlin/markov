//! A generic [Markov chain](https://en.wikipedia.org/wiki/Markov_chain) for almost any type. This
//! uses HashMaps internally, and so Eq and Hash are both required.
//!
//! # Examples
//!
//! ```
//! use markov::Chain;
//!
//! let mut chain = Chain::new();
//! chain.feed_str("I like cats and I like dogs.");
//! println!("{}", chain.generate_str());
//! ```
//!
//! ```
//! use markov::Chain;
//!
//! let mut chain = Chain::new();
//! chain.feed(vec![1u8, 2, 3, 5]).feed(vec![3u8, 9, 2]);
//! println!("{:?}", chain.generate());
//! ```
#![warn(missing_docs)]

extern crate rand;

use std::borrow::ToOwned;
use std::collections::HashMap;
use std::collections::hash_map::Entry::{Occupied, Vacant};
use std::fs::File;
use std::hash::Hash;
use std::io::BufReader;
use std::io::prelude::*;
use std::iter::Map;
use std::path::Path;
use std::rc::Rc;
use rand::{Rng, thread_rng};

/// The definition of all types that can be used in a Chain.
pub trait Chainable: Eq + Hash {}
impl<T> Chainable for T where T: Eq + Hash {}

/// A generic [Markov chain](https://en.wikipedia.org/wiki/Markov_chain) for almost any type. This
/// uses HashMaps internally, and so Eq and Hash are both required.
#[derive(PartialEq, Debug)]
pub struct Chain<T> where T: Chainable {
    map: HashMap<Vec<Option<Rc<T>>>, HashMap<Option<Rc<T>>, usize>>,
    order: usize,
}

impl<T> Chain<T> where T: Chainable {
    /// Constructs a new Markov chain.
    pub fn new() -> Chain<T> {
        Chain {
            map: {
                let mut map = HashMap::new();
                map.insert(vec!(None; 1), HashMap::new());
                map
            },
            order: 1,
        }
    }

    /// Choose a specific Markov chain order. The order is the number of previous tokens to use
    /// as the index into the map.
    pub fn order(&mut self, order: usize) -> &mut Chain<T> {
        assert!(order > 0);
        self.order = order;
        self.map.insert(vec!(None; self.order), HashMap::new());
        self
    }

    /// Determines whether or not the chain is empty. A chain is considered empty if nothing has
    /// been fed into it.
    pub fn is_empty(&self) -> bool {
        self.map[&vec!(None; self.order)].is_empty()
    }


    /// Feeds the chain a collection of tokens. This operation is O(n) where n is the number of
    /// tokens to be fed into the chain.
    pub fn feed(&mut self, tokens: Vec<T>) -> &mut Chain<T> {
        if tokens.len() == 0 { return self }
        let mut toks = vec!(None; self.order);
        toks.extend(tokens.into_iter().map(|token| {
            Some(Rc::new(token))
        }));
        toks.push(None);
        for p in toks.windows(self.order + 1) {
            if !self.map.contains_key(&p[0..self.order].to_vec()) {
                self.map.insert(p[0..self.order].to_vec(), HashMap::new());
            }
            self.map.get_mut(&p[0..self.order].to_vec()).unwrap().add(p[self.order].clone());
        }
        self
    }

    /// Generates a collection of tokens from the chain. This operation is O(mn) where m is the
    /// length of the generated collection, and n is the number of possible states from a given
    /// state.
    pub fn generate(&self) -> Vec<Rc<T>> {
        let mut ret = Vec::new();
        let mut curs = vec!(None; self.order);
        loop {
            let next = self.map[&curs].next();
            curs = curs[1..self.order].to_vec();
            curs.push(next.clone());
            if let Some(next) = next { ret.push(next) };
            if curs[self.order - 1].is_none() { break }
        }
        ret
    }

    /// Generates a collection of tokens from the chain, starting with the given token. This
    /// operation is O(mn) where m is the length of the generated collection, and n is the number
    /// of possible states from a given state. This returns an empty vector if the token is not
    /// found.
    pub fn generate_from_token(&self, token: T) -> Vec<Rc<T>> {
        let token = Rc::new(token);
        if !self.map.contains_key(&vec!(Some(token.clone()); self.order)) { return Vec::new() }
        let mut ret = vec![token.clone()];
        let mut curs = vec!(Some(token.clone()); self.order);
        loop {
            let next = self.map[&curs].next();
            curs = curs[1..self.order].to_vec();
            curs.push(next.clone());
            if let Some(next) = next { ret.push(next) };
            if curs[self.order - 1].is_none() { break }
        }
        ret
    }

    /// Produces an infinite iterator of generated token collections.
    pub fn iter(&self) -> InfiniteChainIterator<T> {
        InfiniteChainIterator { chain: self }
    }

    /// Produces an iterator for the specified number of generated token collections.
    pub fn iter_for(&self, size: usize) -> SizedChainIterator<T> {
        SizedChainIterator { chain: self, size: size }
    }
}

impl Chain<String> {
    /// Feeds a string of text into the chain.
    pub fn feed_str(&mut self, string: &str) -> &mut Chain<String> {
        self.feed(string.split(" ").map(|s| s.to_owned()).collect())
    }

    /// Feeds a properly formatted file into the chain. This file should be formatted such that
    /// each line is a new sentence. Punctuation may be included if it is desired.
    pub fn feed_file(&mut self, path: &Path) -> &mut Chain<String> {
        let reader = BufReader::new(File::open(path).unwrap());
        for line in reader.lines() {
            let line = line.unwrap();
            let words: Vec<_> = line.split_whitespace()
                                    .filter(|word| !word.is_empty())
                                    .collect();
            self.feed(words.iter().map(|&s| s.to_owned()).collect());
        }
        self
    }

    /// Converts the output of generate(...) on a String chain to a single String.
    fn vec_to_string(vec: Vec<Rc<String>>) -> String {
        let mut ret = String::new();
        for s in vec.iter() {
            ret.push_str(&s);
            ret.push_str(" ");
        }
        let len = ret.len();
        if len > 0 {
            ret.truncate(len - 1);
        }
        ret
    }

    /// Generates a random string of text.
    pub fn generate_str(&self) -> String {
        Chain::vec_to_string(self.generate())
    }

    /// Generates a random string of text starting with the desired token. This returns an empty
    /// string if the token is not found.
    pub fn generate_str_from_token(&self, string: &str) -> String {
        Chain::vec_to_string(self.generate_from_token(string.to_owned()))
    }

    /// Produces an infinite iterator of generated strings.
    pub fn str_iter(&self) -> InfiniteChainStringIterator {
        let vec_to_string: fn(Vec<Rc<String>>) -> String = Chain::vec_to_string;
        self.iter().map(vec_to_string)
    }

    /// Produces a sized iterator of generated strings.
    pub fn str_iter_for(&self, size: usize) -> SizedChainStringIterator {
        let vec_to_string: fn(Vec<Rc<String>>) -> String = Chain::vec_to_string;
        self.iter_for(size).map(vec_to_string)
    }
}

/// A sized iterator over a Markov chain of strings.
pub type SizedChainStringIterator<'a> =
Map<SizedChainIterator<'a, String>, fn(Vec<Rc<String>>) -> String>;

/// A sized iterator over a Markov chain.
pub struct SizedChainIterator<'a, T: Chainable + 'a> {
    chain: &'a Chain<T>,
    size: usize,
}

impl<'a, T> Iterator for SizedChainIterator<'a, T> where T: Chainable + 'a {
    type Item = Vec<Rc<T>>;
    fn next(&mut self) -> Option<Vec<Rc<T>>> {
        if self.size > 0 {
            self.size -= 1;
            Some(self.chain.generate())
        } else {
            None
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.size, Some(self.size))
    }
}


/// An infinite iterator over a Markov chain of strings.
pub type InfiniteChainStringIterator<'a> =
Map<InfiniteChainIterator<'a, String>, fn(Vec<Rc<String>>) -> String>;

/// An infinite iterator over a Markov chain.
pub struct InfiniteChainIterator<'a, T: Chainable + 'a> {
    chain: &'a Chain<T>
}

impl<'a, T> Iterator for InfiniteChainIterator<'a, T> where T: Chainable + 'a {
    type Item = Vec<Rc<T>>;
    fn next(&mut self) -> Option<Vec<Rc<T>>> {
        Some(self.chain.generate())
    }
}

/// A collection of states for the Markov chain.
trait States<T: PartialEq> {
    /// Adds a state to this states collection.
    fn add(&mut self, token: Option<Rc<T>>);
    /// Gets the next state from this collection of states.
    fn next(&self) -> Option<Rc<T>>;
}

impl<T> States<T> for HashMap<Option<Rc<T>>, usize> where T: Chainable {
    fn add(&mut self, token: Option<Rc<T>>) {
        match self.entry(token) {
            Occupied(mut e) => *e.get_mut() += 1,
            Vacant(e) => { e.insert(1); },
        }
    }

    fn next(&self) -> Option<Rc<T>> {
        let mut sum = 0;
        for &value in self.values() {
            sum += value;
        }
        let mut rng = thread_rng();
        let cap = rng.gen_range(0, sum);
        sum = 0;
        for (key, &value) in self.iter() {
            sum += value;
            if sum > cap {
                return key.clone()
            }
        }
        unreachable!("The random number generator failed.")
    }
}

#[cfg(test)]
mod test {
    use super::Chain;

    #[test]
    fn new() {
        Chain::<u8>::new();
        Chain::<String>::new();
    }

    #[test]
    fn is_empty() {
        let mut chain = Chain::new();
        assert!(chain.is_empty());
        chain.feed(vec![1u8, 2, 3]);
        assert!(!chain.is_empty());
    }

    #[test]
    fn feed() {
        let mut chain = Chain::new();
        chain.feed(vec![3, 5, 10]).feed(vec![5, 12]);
    }

    #[test]
    fn generate() {
        let mut chain = Chain::new();
        chain.feed(vec![3u8, 5, 10]).feed(vec![5, 12]);
        let v = chain.generate().into_iter().map(|v| *v).collect();
        assert!([vec![3, 5, 10], vec![3, 5, 12], vec![5, 10], vec![5, 12]].contains(&v));
    }

    #[test]
    fn generate_for_higher_order() {
        let mut chain = Chain::new();
        chain.order(2);
        chain.feed(vec![3u8, 5, 10]).feed(vec![2, 3, 5, 12]);
        let v = chain.generate().into_iter().map(|v| *v).collect();
        assert!([vec![3, 5, 10], vec![3, 5, 12], vec![2, 3, 5, 10], vec![2, 3, 5, 12]].contains(&v));
    }

    #[test]
    fn generate_from_token() {
        let mut chain = Chain::new();
        chain.feed(vec![3u8, 5, 10]).feed(vec![5, 12]);
        let v = chain.generate_from_token(5).into_iter().map(|v| *v).collect();
        assert!([vec![5, 10], vec![5, 12]].contains(&v));
    }

    #[test]
    fn generate_from_unfound_token() {
        let mut chain = Chain::new();
        chain.feed(vec![3u8, 5, 10]).feed(vec![5, 12]);
        let v: Vec<_> = chain.generate_from_token(9).into_iter().map(|v| *v).collect();
        assert_eq!(v, vec![]);
    }

    #[test]
    fn iter() {
        let mut chain = Chain::new();
        chain.feed(vec![3u8, 5, 10]).feed(vec![5, 12]);
        assert_eq!(chain.iter().size_hint().1, None);
    }

    #[test]
    fn iter_for() {
        let mut chain = Chain::new();
        chain.feed(vec![3u8, 5, 10]).feed(vec![5, 12]);
        assert_eq!(chain.iter_for(5).collect::<Vec<_>>().len(), 5);
    }

    #[test]
    fn feed_str() {
        let mut chain = Chain::new();
        chain.feed_str("I like cats and dogs");
    }

    #[test]
    fn generate_str() {
        let mut chain = Chain::new();
        chain.feed_str("I like cats").feed_str("I hate cats");
        assert!(["I like cats", "I hate cats"].contains(&&chain.generate_str()[..]));
    }

    #[test]
    fn generate_str_from_token() {
        let mut chain = Chain::new();
        chain.feed_str("I like cats").feed_str("cats are cute");
        assert!(["cats", "cats are cute"].contains(&&chain.generate_str_from_token("cats")[..]));
    }

    #[test]
    fn generate_str_from_unfound_token() {
        let mut chain = Chain::new();
        chain.feed_str("I like cats").feed_str("cats are cute");
        assert_eq!(chain.generate_str_from_token("test"), "");
    }

    #[test]
    fn str_iter() {
        let mut chain = Chain::new();
        chain.feed_str("I like cats and I like dogs");
        assert_eq!(chain.str_iter().size_hint().1, None);
    }

    #[test]
    fn str_iter_for() {
        let mut chain = Chain::new();
        chain.feed_str("I like cats and I like dogs");
        assert_eq!(chain.str_iter_for(5).collect::<Vec<_>>().len(), 5);
    }
}
