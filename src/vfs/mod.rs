mod directory;
mod storage;
mod textfile;
mod types;

use itertools::Itertools;
use std::cell::RefCell;
use std::path::Component;
use std::rc::Rc;

use crate::cli::CommandType;
use crate::vfs::directory::Directory;

pub struct VirtualFilesystem {
    root_dir: Rc<RefCell<Directory>>,
    current_dir: Rc<RefCell<Directory>>
}

impl VirtualFilesystem {
    pub fn new() -> VirtualFilesystem {
        let root = Directory::new_root_directory();
        VirtualFilesystem { root_dir: Rc::clone(&root), current_dir: root }
    }

    pub fn handle_command(&mut self, cmd: CommandType) -> Result<(), String> {
        let mut result: Result<(), String> = Ok(());
        match cmd {
            CommandType::Exit => { result = Err("Command type: exit not supported by virtual filesystem module".to_string()); }
            CommandType::Cd(p) => { self.change_directory(p)?; }
            CommandType::Ls(p) => { self.list_contents(p)?;}
            CommandType::Touch(p) => { self.make_files(p)?;}
            CommandType::Rm(p) => { self.remove(p)?;}
            CommandType::Mkdir(p) => { self.make_directories(p)?; }
            CommandType::Read(p) => { self.read_files(p)?; }
            CommandType::Write(v) => { self.write_data_to_file(v.0, v.1)?; }
            CommandType::Save(p) => { self.save_filelsystem(p)?; }
            CommandType::Load(p) => { self.load_filesystem(p)?; }
        }

        result
    }

    pub fn print_path(&self) {
        let mut dirs = Vec::<String>::new();
        let mut dir = Rc::clone(&self.current_dir);
        dirs.push(dir.borrow().get_name().to_string());
        loop {
            let parent_dir = dir.borrow().get_parent_directory();
            if parent_dir.is_none() {
                break;
            }
            else {
                dir = Rc::clone(&parent_dir.unwrap());
                dirs.push(dir.borrow().get_name().to_string());
            }
        }
        
        print!("/{}", format!("{}", dirs.iter().rev().skip(1).join("/")));
    }

    fn change_directory(&mut self, path: std::path::PathBuf) -> Result<(), String> {
        for component in path.components() {
            match component {
                Component::Prefix(p) => { println!("Prefix: {:?} is ignored (prefixes not supported", p);}
                Component::CurDir => { }
                Component::RootDir => {
                    loop {
                        let parent_dir = self.current_dir.borrow().get_parent_directory();
                        if parent_dir.is_none() {
                            break;
                        }
                        else {
                            self.current_dir = parent_dir.unwrap();
                        }
                    }
                }
                Component::Normal(p) => {
                    let p_str = p.to_str().expect("Failed to extract str for component");
                    let dir = self.current_dir.borrow().get_directory(p_str);
                    if dir.is_none() {
                        return Err(format!("cd: {}: No such directory", p_str));
                    }
                    self.current_dir = dir.unwrap();
                }
                Component::ParentDir => {
                    let parent_dir = self.current_dir.borrow().get_parent_directory();
                    if parent_dir.is_some() {
                        self.current_dir = parent_dir.unwrap();
                    }
                }
            }
        }

        Ok(())
    }

    fn make_directories(&self, names: Vec<std::path::PathBuf>) -> Result<(), String> {
        let mut err = String::new();
        
        for name in names {
            let mut current_dir = Rc::clone(&self.current_dir);
            for component in name.components() {
                match component {
                    Component::Prefix(_) => { err.push_str("mkdir: prefixes not supported"); }
                    Component::CurDir => { err.push_str("mkdir: passed current directory mark - ignoring"); }
                    Component::RootDir => { err.push_str("mkdir: passed root directory mark - ignoring"); }
                    Component::ParentDir => { err.push_str("mkdir: passed parent directory mark - ignoring"); }
                    Component::Normal(p) => {
                        let component_str = p.to_str().expect("Failed to convert name to str");
                        let parent = Rc::clone(&current_dir);
                        let add_dir_result = current_dir.borrow_mut().add_dir(component_str, &parent);
                        if let Err(msg) =  add_dir_result{
                            err.push_str(format!("mkdir: {}\n", msg).as_str());
                            break;
                        }
                        current_dir = add_dir_result.unwrap();
                    }
                }
            }
        }

        if err.is_empty() {
            return Ok(());
        }
        else {
            err.pop();
            return Err(err);
        }
    }

    fn list_contents(&self, mut names: Vec<std::path::PathBuf>) -> Result<(), String> {
        let mut err = String::new();
        if names.is_empty() {
            names.push(std::path::PathBuf::from(".".to_string()));
        }
        for name in names {
            let mut current_dir = Rc::clone(&self.current_dir);
            for component in name.components() {
                match component {
                    Component::Prefix(_) => { err.push_str("ls: prefixes not supported\n"); }
                    Component::CurDir => { }
                    Component::RootDir => { 
                        loop {
                            let parent_dir = current_dir.borrow().get_parent_directory();
                            if parent_dir.is_none() {
                                break;
                            }
                            else {
                                current_dir = parent_dir.unwrap();
                            }
                        }
                     }
                    Component::ParentDir => {
                        let parent_dir = current_dir.borrow().get_parent_directory();
                        if parent_dir.is_none() {
                            break;
                        }
                        else {
                            current_dir = parent_dir.unwrap();
                        }
                     }
                    Component::Normal(p) => {
                        let component_str = p.to_str().expect("Failed to convert name to str");
                        let get_dir_result = current_dir.borrow().get_directory(component_str);
                        if get_dir_result.is_none(){
                            err.push_str(format!("ls: cannot access '{}': no such directory\n", component_str).as_str());
                            break;
                        }
                        current_dir = get_dir_result.unwrap();
                    }
                }
            }
            println!("{}:\n{}", name.to_str().expect("Failed to extract str from name"), current_dir.borrow().get_contents_string());

            }
        if err.is_empty() {
            return Ok(());
        }
        else {
            err.pop();
            return Err(err);
        }
    }

    fn make_files(&self, names: Vec<std::path::PathBuf>) -> Result<(), String> {
        let mut err = String::new();
        for name in names {
            let file_name: &str;
            match name.components().rev().next() {
                Some(Component::Normal(p)) => {
                    file_name = p.to_str().expect("Failed to extract file name from path");
                }
                Some(_) => { err.push_str(format!("touch: {} last element should be file name", name.to_str().expect("Failed to extract str from path")).as_str()); continue; }
                None => { err.push_str("touch: couldn't get last element from path"); continue; }
            }

            let mut current_dir = Rc::clone(&self.current_dir);
            for component in name.components().collect::<Vec<_>>().iter().take(name.components().count() - 1) {
                match component {
                    Component::Prefix(_) => { err.push_str("touch: prefixes not supported\n"); }
                    Component::CurDir => { }
                    Component::RootDir => { 
                        loop {
                            let parent_dir = current_dir.borrow().get_parent_directory();
                            if parent_dir.is_none() {
                                break;
                            }
                            else {
                                current_dir = parent_dir.unwrap();
                            }
                        }
                     }
                    Component::ParentDir => {
                        let parent_dir = current_dir.borrow().get_parent_directory();
                        if parent_dir.is_none() {
                            break;
                        }
                        else {
                            current_dir = parent_dir.unwrap();
                        }
                     }
                    Component::Normal(p) => {
                        let component_str = p.to_str().expect("Failed to convert name to str");
                        let get_dir_result = current_dir.borrow().get_directory(component_str);
                        if get_dir_result.is_none(){
                            err.push_str(format!("touch: cannot access '{}': no such directory\n", component_str).as_str());
                            break;
                        }
                        current_dir = get_dir_result.unwrap();
                    }
                }
            }
            if let Err(msg) = current_dir.borrow_mut().add_file(file_name) {
                err.push_str(&format!("touch: {}\n", msg));
            };
            }
        if err.is_empty() {
            return Ok(());
        }
        else {
            err.pop();
            return Err(err);
        }
    }

    fn remove(&self, names: Vec<std::path::PathBuf>) -> Result<(), String> {
        let mut err = String::new();
        for name in names {
            let file_or_dir_name: &str;
            match name.components().rev().next() {
                Some(Component::Normal(p)) => {
                    file_or_dir_name = p.to_str().expect("Failed to extract file or directory name from path");
                }
                Some(_) => { err.push_str(format!("rm: {} last element should be file or directory name", name.to_str().expect("Failed to extract str from path")).as_str()); continue; }
                None => { err.push_str("rm: couldn't get last element from path"); continue; }
            }

            let mut current_dir = Rc::clone(&self.current_dir);
            for component in name.components().collect::<Vec<_>>().iter().take(name.components().count() - 1) {
                match component {
                    Component::Prefix(_) => { err.push_str("rm: prefixes not supported\n"); }
                    Component::CurDir => { }
                    Component::RootDir => { 
                        loop {
                            let parent_dir = current_dir.borrow().get_parent_directory();
                            if parent_dir.is_none() {
                                break;
                            }
                            else {
                                current_dir = parent_dir.unwrap();
                            }
                        }
                     }
                    Component::ParentDir => {
                        let parent_dir = current_dir.borrow().get_parent_directory();
                        if parent_dir.is_none() {
                            break;
                        }
                        else {
                            current_dir = parent_dir.unwrap();
                        }
                     }
                    Component::Normal(p) => {
                        let component_str = p.to_str().expect("Failed to convert name to str");
                        let get_dir_result = current_dir.borrow().get_directory(component_str);
                        if get_dir_result.is_none(){
                            err.push_str(format!("rm: cannot access '{}': no such directory\n", component_str).as_str());
                            break;
                        }
                        current_dir = get_dir_result.unwrap();
                    }
                }
            }
            if let Err(msg) = current_dir.borrow_mut().remove(file_or_dir_name) {
                err.push_str(&format!("rm: {}\n", msg));
            };
            }
        if err.is_empty() {
            return Ok(());
        }
        else {
            err.pop();
            return Err(err);
        }
    }

    fn read_files(&self, names: Vec<std::path::PathBuf>) -> Result<(), String> {
        let mut err = String::new();
        for name in names {
            let file_name: &str;
            match name.components().rev().next() {
                Some(Component::Normal(p)) => {
                    file_name = p.to_str().expect("Failed to extract file or directory name from path");
                }
                Some(_) => { err.push_str(format!("read: {} last element should be file or directory name", name.to_str().expect("Failed to extract str from path")).as_str()); continue; }
                None => { err.push_str("read: couldn't get last element from path"); continue; }
            }

            let mut current_dir = Rc::clone(&self.current_dir);
            for component in name.components().collect::<Vec<_>>().iter().take(name.components().count() - 1) {
                match component {
                    Component::Prefix(_) => { err.push_str("read: prefixes not supported\n"); }
                    Component::CurDir => { }
                    Component::RootDir => { 
                        loop {
                            let parent_dir = current_dir.borrow().get_parent_directory();
                            if parent_dir.is_none() {
                                break;
                            }
                            else {
                                current_dir = parent_dir.unwrap();
                            }
                        }
                     }
                    Component::ParentDir => {
                        let parent_dir = current_dir.borrow().get_parent_directory();
                        if parent_dir.is_none() {
                            break;
                        }
                        else {
                            current_dir = parent_dir.unwrap();
                        }
                     }
                    Component::Normal(p) => {
                        let component_str = p.to_str().expect("Failed to convert name to str");
                        let get_dir_result = current_dir.borrow().get_directory(component_str);
                        if get_dir_result.is_none(){
                            err.push_str(format!("read: cannot access '{}': no such directory\n", component_str).as_str());
                            break;
                        }
                        current_dir = get_dir_result.unwrap();
                    }
                }
            }

            match current_dir.borrow_mut().get_file(file_name) {
                None => {err.push_str(&format!("read: file {} not found\n", file_name)); }
                Some(f) => { println!("{}:\n{}", file_name, f.borrow().read()); }
            };
            }
        if err.is_empty() {
            return Ok(());
        }
        else {
            err.pop();
            return Err(err);
        }
    }

    fn write_data_to_file(&self, name: std::path::PathBuf, data: String) -> Result<(), String> {
        let file_name: &str;
            match name.components().rev().next() {
                Some(Component::Normal(p)) => {
                    file_name = p.to_str().expect("Failed to extract file or directory name from path");
                }
                Some(_) => { return Err(format!("write: {} last element should be file or directory name", name.to_str().expect("Failed to extract str from path"))); }
                None => { return Err("write: couldn't get last element from path".to_string()); }
            }

            let mut current_dir = Rc::clone(&self.current_dir);
            for component in name.components().collect::<Vec<_>>().iter().take(name.components().count() - 1) {
                match component {
                    Component::Prefix(_) => { return Err("write: prefixes not supported\n".to_string()); }
                    Component::CurDir => { }
                    Component::RootDir => { 
                        loop {
                            let parent_dir = current_dir.borrow().get_parent_directory();
                            if parent_dir.is_none() {
                                break;
                            }
                            else {
                                current_dir = parent_dir.unwrap();
                            }
                        }
                     }
                    Component::ParentDir => {
                        let parent_dir = current_dir.borrow().get_parent_directory();
                        if parent_dir.is_none() {
                            break;
                        }
                        else {
                            current_dir = parent_dir.unwrap();
                        }
                     }
                    Component::Normal(p) => {
                        let component_str = p.to_str().expect("Failed to convert name to str");
                        let get_dir_result = current_dir.borrow().get_directory(component_str);
                        if get_dir_result.is_none(){
                            return Err(format!("write: cannot access '{}': no such directory\n", component_str));
                        }
                        current_dir = get_dir_result.unwrap();
                    }
                }
            }

            match current_dir.borrow_mut().get_file(file_name) {
                None => {return Err(format!("write: file {} not found\n", file_name)); }
                Some(f) => { f.borrow_mut().append(data.as_str()); }
            };
            Ok(())
            }

    fn save_filelsystem(&self, name: std::path::PathBuf) -> Result<(), String> {
        if self.current_dir.borrow().get_parent_directory().is_none() {
            storage::store_filesystem_in_file(name, Rc::clone(&self.current_dir))?;
        }
        else {
            let mut current_dir = Rc::clone(&self.current_dir);
        loop {
            let parent_dir = current_dir.borrow().get_parent_directory();
            if parent_dir.is_none() {
                break;
            }
            else {
                current_dir = Rc::clone(&parent_dir.unwrap());
            }
        }
        storage::store_filesystem_in_file(name, Rc::clone(&current_dir))?;
        }
        Ok(())
    }

    fn load_filesystem(&mut self, name: std::path::PathBuf) -> Result<(), String> {
        self.root_dir = storage::create_filesystem_from_file(name)?;
        self.current_dir = Rc::clone(&self.root_dir);
        Ok(())
    }
}