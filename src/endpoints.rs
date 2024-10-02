use std::fmt;
use std::fs::{write, File};
use std::io::{BufWriter, Write};
use std::ops::Add;
use std::path::Path;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug,Deserialize, Serialize, Clone)]
struct Character {
    id: usize,
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



// returns the entries from 0 to limit
// returns everything if limit is set to 0
pub(crate) fn get_entries(limit:usize) -> String {
    let file_path = Path::new("one_piece2.json");
    let file = File::open(file_path).expect("Failed to open file");
    let characters:Vec<Character> = serde_json::from_reader(file)
        .expect("Error while parsing");

    let response: String;

    if(limit == 0){

        response = serde_json::to_string(&characters).expect("Error parsing to string")
    }
    else{
        response = serde_json::to_string(&characters[0..limit]).expect("Error parsing to string");
    }


    return response
}

//appends a new entry to the end of the .json file
pub(crate) fn post_entry(req: &str) -> &str{

    let req:Result<Character, serde_json::Error> = serde_json::from_str(req);
    match req{
        Ok(mut new_character) =>{
            let file_path = Path::new("one_piece2.json");
            let file = File::open(file_path).expect("Failed to open file");
            let mut characters:Vec<Character> = serde_json::from_reader(file)
                .expect("Error while parsing");
            new_character.id = characters.last().unwrap().id+1;
            characters.push(new_character);

            let new_req = serde_json::to_string_pretty(&characters).unwrap();
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
    "Success!"
}

//replaces all the fields of a selected entry filtered by id
pub(crate) fn put_entry(req: &str) -> &str {
    
    let patched_entry:Result<Character, serde_json::Error> = serde_json::from_str(req);
    match patched_entry{
        Ok(mut new_character) =>{
            let file_path = Path::new("one_piece2.json");
            let file = File::open(file_path).expect("Failed to open file");
            let mut characters:Vec<Character> = serde_json::from_reader(file)
                .expect("Error while parsing");
            let mut flag:bool = false;
            let mut index:usize = 0;
            for mut character in characters.clone(){
                if(character.id == new_character.id){
                    flag = true;
                    break;
                }
                index+=1;
            }

            if(!flag) { return "Error"; }
            characters.insert(index, new_character);
            characters.remove(index+1);

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

//patches the name field of an entry and replaces it with the name new name field
pub(crate) fn patch_entry_name(req: &str) -> &str {
    #[derive(Deserialize, Clone)]
    struct PatchName{
        id: usize,
        name: String
    }

    let req: Result<PatchName, serde_json::Error> = serde_json::from_str(req);
    match req{
        Ok(patch) => {
            let file_path = Path::new("one_piece2.json");
            let file = File::open(file_path).expect("Failed to open file");
            let mut characters:Vec<Character> = serde_json::from_reader(file)
                .expect("Error while parsing");
            // Find and update the character's name
            if let Some(character) = characters.iter_mut().find(|c| c.id == patch.id) {
                character.name = patch.name.clone();
            } else {
                return "Character not found";
            }

            let file = File::create(file_path).unwrap();
            let mut writer = BufWriter::new(file);
            serde_json::to_writer_pretty(&mut writer, &characters).unwrap();

            // Optionally, add a newline for better formatting
            writer.write_all(b"\n").unwrap();


        },
        Err(e) => {
            return "Format not valid";
        }
    }
    "Success"
}

//removes an entry from the .json file
pub(crate) fn delete_entry(req: &str) -> &str {
    #[derive(Deserialize)]
    struct Delete{
        id: usize
    }

    let req:Result<Delete, serde_json::Error> = serde_json::from_str(req);
    match req{
        Ok(delete_req) => {
            let file_path = Path::new("one_piece2.json");
            let file = File::open(file_path).expect("Failed to open file");
            let mut characters:Vec<Character> = serde_json::from_reader(file)
                .expect("Error while parsing");

            let index: Option<usize> = characters.iter().position(|&r| r.id==delete_req.id);
            match index{
                Ok(element_index) => {
                    characters.remove(element_index);
                },
                Err(e) => {
                    return "Error"
                }
            }

            let file = File::create(file_path).unwrap();
            let mut writer = BufWriter::new(file);
            serde_json::to_writer_pretty(&mut writer, &characters).unwrap();

            // Optionally, add a newline for better formatting
            writer.write_all(b"\n").unwrap();

        }
        Err(e) => {
            return "Error"
        }
    }
    "Success!"
}