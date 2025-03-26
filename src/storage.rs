use std::collections::HashMap;
use std::io::{Write, Read};

pub struct Storage {
    map: HashMap<String, String>,
    file: Option<std::path::PathBuf>,
    update_file: bool,
}

impl Storage {
    #[cfg(test)]
    fn temporary() -> Self {
        println!("Created temporary storage");
        Storage{map: HashMap::<String, String>::new(), file: None, update_file: false }
    }

    pub fn new() -> Self {
        println!("Created a new empty storage");
        Storage{map: HashMap::<String, String>::new(), file: Some(std::path::PathBuf::from("storage.db")), update_file: false }
    
    }

    pub fn from_db(path: &str) -> Self {
        let f = std::fs::File::open(path).expect("Failed to open file");
        let mut reader = std::io::BufReader::new(f);
        let mut content = String::new();
        reader.read_to_string(&mut content).expect("Failed to read data");
        let loaded_map: HashMap<String, String> = serde_json::from_str(&content).expect("Failed to deserialize data from DB");
        println!("Created a storage from file {}", path);
        Storage{map: loaded_map, file: Some(std::path::PathBuf::from(path)), update_file: false }
    }

    pub fn insert(&mut self, key: String, val: String) -> Result<(), String>{
        match self.map.entry(key) {
            std::collections::hash_map::Entry::Vacant(entry) => {
                entry.insert(val);
                self.update_file = true;
                Ok(())
            }
            std::collections::hash_map::Entry::Occupied(entry) => {
                Err(format!("Key: {} already exists", entry.key()))
            }
        }
    }

    pub fn delete(&mut self, key: String) -> Result<(), String>{
        match self.map.remove(&key) {
            Some(_) => { self.update_file = true; Ok(())}
            None => { Err(format!("Key: {} not found is storage", key)) }
        }
    }

    pub fn get(&self, key: String) -> Result<&String, String>{
        match self.map.get(&key) {
            Some(v) => { Ok(v) }
            None => { Err(format!("Key {} not found in storage", key)) }
        }
    }

    pub fn list(&self) -> Vec<(&String, &String)> {
        self.map.iter().collect()
    }
}

impl Drop for Storage {
    fn drop(&mut self) {
        if self.update_file {
            if let Some(ref path) = self.file {
                let f = std::fs::File::create(path);
                match f {
                    Err(msg) => {
                        panic!("Error during opening file: {:?}. Error msg: {}", path, msg);
                    }
                    Ok(mut f) => {
                        let serialized_map = serde_json::to_string(&self.map).expect("Error during serializing");
                        f.write_all(serialized_map.as_bytes()).expect("Saving data to file failed.");
                    }
                }
            }    
        }
    }
}

#[test]
fn storage_methods_tests() {
    let mut storage = Storage::temporary();
    let _ = storage.insert("k1".to_string(), "v1".to_string());
    let _ = storage.insert("k2".to_string(), "v2".to_string());
    let _ = storage.insert("k3".to_string(), "v3".to_string());
    let _ = storage.insert("k4".to_string(), "v4".to_string());
    let _ = storage.insert("k5".to_string(), "v5".to_string());
    let _ = storage.insert("k6".to_string(), "v5".to_string());

    {
        let result = storage.insert("k2".to_string(), "v2".to_string());
        assert!(result.is_err());
    }
    {
        let result = storage.get("k3".to_string());
        assert!(result.is_ok());
        assert!(result.unwrap() == "v3");
    }
    {
        let mut result = storage.delete("k4".to_string());
        assert!(result.is_ok());
        result = storage.delete("k4".to_string());
        assert!(result.is_err());
    }
    {
        let mut result = storage.list();
        let pattern = vec![("k1".to_string(), "v1".to_string()), ("k2".to_string(), "v2".to_string()), ("k3".to_string(), "v3".to_string()), ("k5".to_string(), "v5".to_string()), ("k6".to_string(), "v5".to_string()),];
        assert!(result.len() == pattern.len());

        result.sort_by(|a, b| a.0.cmp(&b.0));
        for i in 0..result.len() {
            let (r_first, r_second) = &result[i];
            let (p_first, p_second) = &pattern[i];
            assert!(*r_first == p_first);
            assert!(*r_second == p_second);
        }
    }

    assert!(storage.file.is_none());
}

#[test]
fn flag_test () {
    {
        let mut storage = Storage::temporary();
        assert!(storage.update_file == false);
        let _ = storage.insert("k1".to_string(), "v1".to_string());
        assert!(storage.update_file == true);
    }
    {
        let mut storage = Storage::temporary();
        storage.map.insert("dd".to_string(), "aa".to_string());
        assert!(storage.update_file == false);
        let _ = storage.delete("k1".to_string());
        assert!(storage.update_file == false);
        let _ = storage.delete("dd".to_string());
        assert!(storage.update_file == true);
    }
    {
        let mut storage = Storage::temporary();
        storage.map.insert("dd".to_string(), "aa".to_string());
        assert!(storage.update_file == false);
        let _ = storage.insert("dd".to_string(), "v1".to_string());
        assert!(storage.update_file == false);
    }
    {
        let mut storage = Storage::temporary();
        storage.map.insert("dd".to_string(), "aa".to_string());
        storage.map.insert("bb".to_string(), "cc".to_string());
        assert!(storage.update_file == false);
        let _ = storage.get("dd".to_string());
        let _ = storage.list();
        assert!(storage.update_file == false);
    }
}