use std::cmp::PartialOrd;
use std::collections::LinkedList;

pub struct Node<T: PartialOrd + Copy + ToString> {
    key: T,
    text: String,
    left: BinaryTree<T>,
    right: BinaryTree<T>,
}

pub enum BinaryTree<T: PartialOrd + Copy + ToString> {
    Empty,
    NonEmpty(Box<Node<T>>),
}

impl<T:PartialOrd + Copy + ToString> BinaryTree<T>{
    pub fn insert(&mut self, k:T, t:String) -> Result<(), String>  {
        match self {
            BinaryTree::Empty => {
                *self = BinaryTree::NonEmpty(Box::new(Node{key: k, text: t, left: BinaryTree::Empty, right: BinaryTree::Empty}));
                Ok(())
            },
            BinaryTree::NonEmpty(ref mut node) => {
                if k < node.key { node.left.insert(k, t)?; Ok(()) }
                else if k > node.key { node.right.insert(k, t)?; Ok(()) }
                else { Err("Key already exists".to_string()) }
            }
        }
    }

    pub fn search(&self, k:T) -> Option<(T, &str)> {
        match self {
            BinaryTree::Empty => { return None; }
            BinaryTree::NonEmpty(ref node) => {
                if k == node.key { Some((node.key, &node.text)) }
                else if k < node.key { node.left.search(k) }
                else { node.right.search(k) }
            }
        }
    }

    pub fn serialize_tree(&self) -> String {
        fn fill_tree_map<T:PartialOrd + Copy + ToString>(mut tree_map: &mut LinkedList<String>, tree: &BinaryTree<T>) {
            match tree {
                BinaryTree::Empty => {},
                BinaryTree::NonEmpty(ref child) => {
                    let mut details: String = format!("[k:{},t:{}", child.key.to_string(), child.text);
                    if let BinaryTree::NonEmpty(ref left_child) = child.left {
                        details = format!("{},L:{}", details, left_child.key.to_string());
                    }
                    if let BinaryTree::NonEmpty(ref right_child) = child.right {
                        details = format!("{},R:{}", details, right_child.key.to_string());
                    }
                    details.push(']');
                    tree_map.push_back(details);
                    fill_tree_map(&mut tree_map, &child.left);
                    fill_tree_map(&mut tree_map, &child.right);
                }
            }
        }

        let mut tree_map: LinkedList<String> = LinkedList::new();
        fill_tree_map(&mut tree_map, &self);
        tree_map.iter().map(|s| s.as_str()).collect::<Vec<_>>().join(";")
    }
}

#[test]
fn test_tree(){
    let empty_tree = BinaryTree::<u32>::Empty;
    assert!(empty_tree.serialize_tree() == "".to_string());
    assert!(empty_tree.search(5) == None);

    let mut example_tree = BinaryTree::<u32>::Empty;
    let _ = example_tree.insert(4, "root".to_string());
    let _ = example_tree.insert(10, "a".to_string());
    let _ = example_tree.insert(3, "b".to_string());
    let _ = example_tree.insert(1, "c".to_string());
    let _ = example_tree.insert(13, "d".to_string());
    let _ = example_tree.insert(5, "e".to_string());
    let _ = example_tree.insert(55, "f".to_string());
    let _ = example_tree.insert(30, "g".to_string());
    assert!(example_tree.search(100) == None);
    assert!(example_tree.search(55) == Some((55, "f")));
    assert!(example_tree.serialize_tree() == "[k:4,t:root,L:3,R:10];[k:3,t:b,L:1];[k:1,t:c];[k:10,t:a,L:5,R:13];[k:5,t:e];[k:13,t:d,R:55];[k:55,t:f,L:30];[k:30,t:g]");
    let result = example_tree.insert(5, "something".to_string());
    assert!(result.is_err());
}