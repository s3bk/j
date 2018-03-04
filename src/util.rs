use serde::*;
use bincode::*;
use std::fs::File;
use std::io::{BufReader, BufWriter};

pub fn load<T>(path: &str) -> T
    where T: for <'de> Deserialize<'de> + Default + Sized
{
    let mut buf = BufReader::new(File::open(path).expect("could not open file"));
    deserialize_from(&mut buf).unwrap_or_else(|_| T::default())
}
pub fn save<T: Serialize>(path: &str, data: &T) {
    let mut buf = BufWriter::new(File::create(path).expect("could not create file"));
    serialize_into(&mut buf, data).expect("Invalid data");
}
