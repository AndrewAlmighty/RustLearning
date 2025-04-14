use crate::vfs::directory::Directory;
use crate::vfs::types::NameType;
use crate::vfs::textfile::TextFile;

use std::cell::RefCell;
use std::rc::Rc;

use std::io::{Write, Read};

use serde::{Serialize, Deserialize};

#[derive(Serialize, Deserialize)]
struct SerializableDirectory {
    name: String,
    files: Vec<(String, String)>, // (filename, contents)
    subdirectories: Vec<SerializableDirectory>,
}

impl SerializableDirectory {
    fn from_directory(dir: Rc<RefCell<Directory>>) -> SerializableDirectory {
        let mut sd = SerializableDirectory{ name: String::from(dir.borrow().get_name()), files: Vec::new(), subdirectories: Vec::new() };

        for file in dir.borrow().get_files() {
            let bfile = file.borrow();
            sd.files.push((String::from(bfile.get_name()), String::from(bfile.read())));
        }

        for d in dir.borrow().get_directories() {
            sd.subdirectories.push(SerializableDirectory::from_directory(Rc::clone(d)));
        }

        sd
    }

    fn to_directory(&self, current_dir: Rc<RefCell<Directory>>) -> Result<(), String> {
        for file in &self.files {
            let f = Rc::new(RefCell::new(TextFile::with_data(NameType::new(file.0.clone()), file.1.as_str())?));
            current_dir.borrow_mut().add_complete_file(&f)?;
        }

        for directory in &self.subdirectories {
            let new_dir = current_dir.borrow_mut().add_dir(&directory.name, &current_dir)?;
            directory.to_directory(Rc::clone(&new_dir))?;
        }

        Ok(())
    }
}

pub(super) fn create_filesystem_from_file(name: std::path::PathBuf) -> Result<Rc<RefCell<Directory>>, String> {
    let f = std::fs::File::open(name);
    if f.is_err() {
        return Err(format!("Cannot open file: {}", f.unwrap_err()));
    }

    let mut file_content = String::new();
    let mut reader = std::io::BufReader::new(f.unwrap());
    reader.read_to_string(&mut file_content).expect("Failed to read data");
    let serialized_directory: SerializableDirectory = serde_json::from_str(&file_content).expect("Couldn't deserialize json");
    let new_root_directory = Directory::new_root_directory();
    serialized_directory.to_directory(Rc::clone(&new_root_directory))?;
    Ok(new_root_directory)
}

pub(super) fn store_filesystem_in_file(name: std::path::PathBuf, root: Rc<RefCell<Directory>>) -> Result<(), String> {
    if !root.borrow().get_parent_directory().is_none() {
        return Err("Storing filelsystem requires root directory, received directory which has parent".to_string());
    }

    let serialized_directory = SerializableDirectory::from_directory(root);
    let directory_as_json = serde_json::to_string(&serialized_directory).expect("Error during serializing to json");
    
    let mut f = std::fs::File::create(name).expect("Error during creating file");
    f.write_all(directory_as_json.as_bytes()).expect("Savind data to file failed");
    Ok(())
}