use crate::vfs::textfile::TextFile;
use crate::vfs::types::NameType;

use itertools::Itertools;

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::rc::Weak;

pub(super) struct Directory {
    name: NameType,
    files: HashMap<NameType, Rc<RefCell<TextFile>>>,
    subdirectories: HashMap<NameType, Rc<RefCell<Directory>>>,
    parent_directory: Weak<RefCell<Directory>>
}

impl Directory {
    fn name_taken(&self, name: &str) -> Option<String> {
        if self.subdirectories.contains_key(name) {
            return Some(format!("Directory {} already exists", name));
        }
        else if self.files.contains_key(name) {
            return Some(format!("File {} already exists", name));
        }
        None
    }

    pub(super) fn new_root_directory() -> Rc<RefCell<Self>> {
        Rc::new(RefCell::new(Self {
            name: NameType::new("/".to_string()),
            files: HashMap::new(),
            subdirectories: HashMap::new(),
            parent_directory: Weak::new()
        } ))
    }

    pub(super) fn add_file(&mut self, name: &str) -> Result<(), String> {
        if let Some(e) = self.name_taken(name) {
            return Err(e)
        }

        let file_name = NameType::new(name.to_string());
        let f = Rc::new(RefCell::new(TextFile::new(file_name.clone())?));
        self.files.insert(file_name, f);
        Ok(())
    }

    pub(super) fn add_complete_file(&mut self, file: &Rc<RefCell<TextFile>>) -> Result<(), String> {
        let bfile = file.borrow();
        let file_name = bfile.get_name();
        if let Some(e) = self.name_taken(file_name) {
            return Err(e)
        }

        self.files.insert(NameType::new(file_name.to_string()), Rc::clone(&file));
        Ok(())
    }

    pub(super) fn add_dir(&mut self, name: &str, parent: &Rc<RefCell<Directory>>) -> Result<Rc<RefCell<Directory>>, String> {
        if let Some(e) = self.name_taken(name) {
            return Err(e)
        }

        let dir_name = NameType::new(name.to_string());

        let dir = Rc::new(RefCell::new(Directory {
            name: dir_name.clone(),
            files: HashMap::new(),
            subdirectories: HashMap::new(),
            parent_directory: Rc::downgrade(parent) } ));

        self.subdirectories.insert(dir_name.clone(), Rc::clone(&dir));
        Ok(dir)
    }

    pub(super) fn get_contents_string(&self) -> String {
        format!("Directories: [{}], Files: [{}]",
            self.subdirectories
                .iter()
                .map(|(_, dir)| (*dir.borrow().name).to_string())
                .sorted()
                .collect::<Vec<String>>().join(", "),
            self.files
                .iter()
                .map(|(_, file)| file.borrow().get_name().to_string())
                .sorted()
                .collect::<Vec<String>>().join(", ")
            )
    }

    pub(super) fn remove(&mut self, name: &str) -> Result<(), String> {
        if self.files.contains_key(name) {
            self.files.remove(name);
            return Ok(());
        }
        else if self.subdirectories.contains_key(name) {
            self.subdirectories.remove(name);
            return Ok(());
        }

        Err(format!("File or directory not found: {}", name))
    }

    pub(super) fn get_directory(&self, name: &str) -> Option<Rc<RefCell<Directory>>> {

        match self.subdirectories.get(name) {
            Some(dir) => { Some(Rc::clone(&dir)) }
            None => { None }
        }
    }

    pub(super) fn get_file(&self, name: &str) -> Option<Rc<RefCell<TextFile>>> {
        match self.files.get(name) {
            Some(file) => { Some(Rc::clone(&file)) }
            None => { None }
        }
    }

    pub(super) fn get_files(&self) -> impl Iterator<Item = &Rc<RefCell<TextFile>>> {
        self.files.values()
    }

    pub(super) fn get_directories(&self) -> impl Iterator<Item = &Rc<RefCell<Directory>>> {
        self.subdirectories.values()
    }

    pub(super) fn get_name(&self) -> &str {
        &self.name
    }

    pub(super) fn get_parent_directory(&self) -> Option<Rc<RefCell<Directory>>> {
        self.parent_directory.upgrade()
    }
}

#[test]
fn test_directory() {
    let root = Directory::new_root_directory();
    {
        let r_dir = root.borrow();
        assert!(*r_dir.get_name() == *"root");
        assert!(r_dir.get_parent_directory().is_none());
        assert!(r_dir.subdirectories.is_empty());
        assert!(r_dir.files.is_empty());
        assert!(r_dir.get_contents_string() == "Directories: [], Files: []".to_string());
    }
    {
        root.borrow_mut().add_file("A1.txt").expect("File should be created");
        root.borrow_mut().add_file("A2.txt").expect("File should be created");
        let r_err = root.borrow_mut().add_file("A2.txt");
        assert!(r_err.is_err());
        let r_dir = root.borrow();
        assert!(*r_dir.get_name() == *"root");
        assert!(r_dir.get_parent_directory().is_none());
        assert!(r_dir.subdirectories.is_empty());
        assert!(r_dir.files.len() == 2);
        assert!(r_dir.get_contents_string() == "Directories: [], Files: [A1.txt, A2.txt]".to_string());
    }
    {
        root.borrow_mut().add_dir("home", &root).expect("Directory should be created");
        root.borrow_mut().add_dir("downloads", &root).expect("Directory should be created");
        let r_err = root.borrow_mut().add_dir("downloads", &root);
        assert!(r_err.is_err());
        let r_dir = root.borrow();
        assert!(*r_dir.get_name() == *"root");
        assert!(r_dir.get_parent_directory().is_none());
        assert!(r_dir.subdirectories.len() == 2);
        assert!(r_dir.files.len() == 2);
        assert!(r_dir.get_contents_string() == "Directories: [downloads, home], Files: [A1.txt, A2.txt]".to_string());
    }
    {
        root.borrow_mut().remove("home").expect("Directory should be deleted");
        root.borrow_mut().remove("A1.txt").expect("File should be deleted");
        let err_1 = root.borrow_mut().remove("fdszxv");
        let err_2 = root.borrow_mut().remove("A113d.txt");
        assert!(err_1.is_err());
        assert!(err_2.is_err());
        let r_dir = root.borrow();
        assert!(*r_dir.get_name() == *"root");
        assert!(r_dir.get_parent_directory().is_none());
        assert!(r_dir.subdirectories.len() == 1);
        assert!(r_dir.files.len() == 1);
        assert!(r_dir.get_contents_string() == "Directories: [downloads], Files: [A2.txt]".to_string());
    }
    {
        let no_file = root.borrow().get_file("adf.txt");
        assert!(no_file.is_none());
        let no_dir = root.borrow().get_directory("adf");
        assert!(no_dir.is_none());
    }
    {
        root.borrow_mut().add_dir("tmp", &root).expect("Directory should be created");
        let tmp_dir = root.borrow_mut().get_directory("tmp").expect("Directory tmp should be returned");
        tmp_dir.borrow_mut().add_dir("tester", &tmp_dir).expect("Directory should be created");
        let tester_dir = tmp_dir.borrow_mut().get_directory("tester").expect("Directory tester should be returned");
        tester_dir.borrow_mut().add_dir("t1", &tester_dir).expect("Directory should be created");
        tester_dir.borrow_mut().add_dir("t2", &tester_dir).expect("Directory should be created");
        tester_dir.borrow_mut().add_file("notepad.txt").expect("File should be created");
        let notepad_file = tester_dir.borrow().get_file("notepad.txt").expect("notepad file should be returned");
        notepad_file.borrow_mut().append("someasd ");
        notepad_file.borrow_mut().append("dsdf");
        let borrowed_tester_dir = tester_dir.borrow();
        assert!(*borrowed_tester_dir.get_name() == *"tester");
        assert!(*(borrowed_tester_dir.get_parent_directory().expect("There should be parent directory")).borrow().get_name() == *"tmp");
        assert!(borrowed_tester_dir.subdirectories.len() == 2);
        assert!(borrowed_tester_dir.files.len() == 1);
        assert!(borrowed_tester_dir.get_contents_string() == "Directories: [t1, t2], Files: [notepad.txt]".to_string());
        assert!((borrowed_tester_dir.get_file("notepad.txt").expect("There should be notepad.txt file")).borrow().read() == "someasd dsdf");
    }
}