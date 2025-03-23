mod binary_tree;

use std::io::{self, Write};

fn main() {
    let mut tree = binary_tree::BinaryTree::<i32>::Empty;
    println!("Welcome to binary tree manager");
    println!("Available actions:\ninsert - Insert element to tree,\nsearch - Search for element in tree,\nprint- Print serialized tree, \nquit - finishes app.");

    loop {
        print!("Action: ");
        io::stdout().flush().expect("Failed to flush stdout");
        let mut input = String::new();
        io::stdin().read_line(&mut input).expect("Failed to read line");
        input = input.trim().to_string();

        match input.as_str(){
            "quit" => { break; }
            "insert" => {
                input.clear();
                println!("Pass a key (number):");
                io::stdin().read_line(&mut input).expect("Failed to read line");
                input = input.trim().to_string();
                let k:i32 = input.parse().expect("expecting a number");
                input.clear();
                println!("Pass a value (text):");
                io::stdin().read_line(&mut input).expect("Failed to read line");
                input = input.trim().to_string();
                let result = tree.insert(k, input);
                if result.is_err() {
                    println!("Key: {} already exists!", k);
                }
            }
            "search" => {
                println!("Give me a key: ");
                input.clear();
                io::stdin().read_line(&mut input).expect("Failed to read line");
                input = input.trim().to_string();
                let k:i32 = input.parse().expect("expecting a number");
                let result = tree.search(k);
                match result {
                    Some(r) => {
                        println!("Found key: {}, text: {}", r.0, r.1);
                    }
                    None => {
                        println!("Could not find key: {} in binary tree", k);
                    }
                }
            }
            "print" => {
                println!("Serialized version of tree: {}\n\n", tree.serialize_tree());
            }
            &_ => {}
        }
    }

    println!("Goodbye");
}
