use crate::vfs::types::NameType;

pub(super) struct TextFile {
    name: NameType,
    data: String,
}

impl TextFile {
    pub(super) fn new(name: NameType) -> Result<Self, String> {
        if name.is_empty() {
            Err("Name for Textfile cannot be empty".to_string())
        }
        else {
            Ok(TextFile{ name: name, data: "".to_string() })
        }
    }

    pub(super) fn with_data(name: NameType, data: &str) -> Result<Self, String> {
        if name.is_empty() {
            Err("Name for Textfile cannot be empty".to_string())
        }
        else {
            Ok(TextFile{ name: name.clone(), data: data.to_string() })
        }
    }

    pub(super) fn get_name(&self) -> &str {
        &self.name
    }

    pub(super) fn read(&self) -> &str {
        &self.data
    }

    pub(super) fn append(&mut self, data: &str) {
        self.data.push_str(data)
    }
}

#[test]
fn test_textfile() {
    {
        let mut f1 = TextFile::new(NameType::new("moj.txt".to_string())).unwrap();
        assert!(f1.get_name() == "moj.txt");
        assert!(f1.read().is_empty());
        f1.append("newnew");
        assert!(f1.get_name() == "moj.txt");
        assert!(f1.read() == "newnew");
        f1.append("zxcvasdf");
        assert!(f1.get_name() == "moj.txt");
        assert!(f1.read() == "newnewzxcvasdf");
    }
    {
        let mut f2 = TextFile::with_data(NameType::new("mds.md".to_string()), "cos").unwrap();
        assert!(f2.get_name() == "mds.md");
        assert!(f2.read() == "cos");
        f2.append("newnew");
        assert!(f2.get_name() == "mds.md");
        assert!(f2.read() == "cosnewnew");
    }
    {
        let mut f3 = TextFile::with_data(NameType::new("mds.md".to_string()), "").unwrap();
        assert!(f3.get_name() == "mds.md");
        assert!(f3.read().is_empty());
        f3.append("newnew");
        assert!(f3.get_name() == "mds.md");
        assert!(f3.read() == "newnew");
    }
    {
        let wrong1 = TextFile::new(NameType::new("".to_string()));
        assert!(wrong1.is_err());
        let wrong2 = TextFile::with_data(NameType::new("".to_string()), "");
        assert!(wrong2.is_err());
    }
}