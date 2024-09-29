use std::fmt;
use std::fs::{write, File};
use std::io::{BufWriter, Write};
use std::ops::Add;
use std::path::Path;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug,Deserialize, Serialize)]
struct Character {
    id: u32,
    rank: String,
    trend: String,
    season: u32,
    episode: u32,
    name: String,
    start: u32,
    total_votes: String,
    average_rating:f32
}
impl fmt::Display for Character{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:?}", self)
    }
}
pub(crate) fn get_entries(size:usize) -> String {
    let file_path = Path::new("one_piece.json");
    let file = File::open(file_path).expect("Failed to open file");
    let characters:Vec<Character> = serde_json::from_reader(file)
        .expect("Error while parsing");

    let response: String;

    if(size == 0){

        response = serde_json::to_string(&characters).expect("Error parsing to string")
    }
    else{
        response = serde_json::to_string(&characters[0..size]).expect("Error parsing to string");
    }


    return response
}

pub(crate) fn post_entry(data: &str) -> &str{

    let req:Result<Character, serde_json::Error> = serde_json::from_str(data);
    match req{
        Ok(mut new_character) =>{
            let file_path = Path::new("one_piece2.json");
            let file = File::open(file_path).expect("Failed to open file");
            let mut characters:Vec<Character> = serde_json::from_reader(file)
                .expect("Error while parsing");
            new_character.id = characters.last().unwrap().id+1;
            characters.push(new_character);

            let new_data = serde_json::to_string_pretty(&characters).unwrap();
            let file = File::create(file_path).unwrap();
            let mut writer = BufWriter::new(file);
            serde_json::to_writer_pretty(&mut writer, &characters).unwrap();

            // Optionally, add a newline for better formatting
            writer.write_all(b"\n").unwrap();


        },
        Err(e) =>{
            return "Error"
        }
    }
    return "Success!"
}
