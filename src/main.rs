use std::io::{Read, Write};
use regex::Regex;

fn parse_arguments(args: Vec<String>) -> (std::path::PathBuf, std::path::PathBuf) {
    let print_help = || {
        println!("Usage: -i <input.md> -o <output.html>");
        std::process::exit(0);
    };

    let mut input: Option<std::path::PathBuf> = None;
    let mut output: Option<std::path::PathBuf> = None;

    if args.is_empty() {
        print_help();
    }
    else if args.len() != 4 {
        print_help();
    }
    else {
        let mut it = args.iter();
        while let Some(arg) = it.next() {
            match arg.as_str() {
                "-h" | "--help" => { print_help(); }
                "-i" => { input = it.next().map(|s| s.into()); }
                "-o" => { output = it.next().map(|s| s.into()); }
                e => { panic!("Unexpected option: {}", e); }
            }
        }
    }

    assert!(input.is_some());
    assert!(output.is_some());
    (input.unwrap(), output.unwrap())
}

fn convert_md_to_html(md_content: String) -> String {
    let mut html = String::new();

    let re_h = Regex::new(r"^(#{1,2})\s+(.+)$").unwrap();
    let re_ol = Regex::new(r"^\d\.\s(.*)$").unwrap();
    let re_ul = Regex::new(r"^-\s(.*)$").unwrap();

    let re_bold = Regex::new(r"\*\*(.*?)\*\*").unwrap();
    let re_italic = Regex::new(r"\*(.*?)\*").unwrap();
    let re_link = Regex::new(r"\[(.+?)\]\((.+?)\)").unwrap();
    let re_inline_code = Regex::new(r"`(.*?)`").unwrap();

    enum ListType {
        Unordered,
        Ordered,
    }

    let mut list_type: Option<ListType> = None;

    for line in md_content.lines() {
        let mut new_html_line = String::new();

        if let Some(cap_h) = re_h.captures(line) {
            let lvl = cap_h[1].len();
            new_html_line = format!("<h{}>{}</h{}>", lvl, &cap_h[2], lvl);
        }
        else if let Some(ol) = re_ol.captures(line) {
            if list_type.is_none() {
                html.push_str("<ol>\n");
                list_type = Some(ListType::Ordered);
            }
            new_html_line = format!("<li>{}</li>", &ol[1]);
        }
        else if let Some(ul) = re_ul.captures(line) {
            if list_type.is_none() {
                html.push_str("<ul>\n");
                list_type = Some(ListType::Unordered);
            }
            new_html_line = format!("<li>{}</li>", &ul[1]);
        }
        else {
            if let Some(l) = list_type.take() {
                match l {
                    ListType::Unordered => { html.push_str("</ul>"); }
                    ListType::Ordered => { html.push_str("</ol>"); }
                }
            }

            if !line.trim().is_empty() {
                new_html_line = format!("<p>{}</p>", line);
            }
        }

        new_html_line = re_bold.replace_all(&new_html_line, "<b>$1</b>").to_string();
        new_html_line = re_italic.replace_all(&new_html_line, "<i>$1</i>").to_string();
        new_html_line = re_link.replace_all(&new_html_line, "<a href=\"$2\">$1</a>").to_string();
        new_html_line = re_inline_code.replace_all(&new_html_line, "<code>$1</code>").to_string();

        html.push_str(&new_html_line);
        html.push('\n');
    }

    if let Some(l) = list_type {
        match l {
            ListType::Unordered => html.push_str("</ul>\n"),
            ListType::Ordered => html.push_str("</ol>\n"),
        }
    }

    html
}

fn main() {
    
    let html_file: std::fs::File;
    let mut md_content = String::new();

    {
        let in_out_files = parse_arguments(std::env::args().skip(1).collect());
        let mut md_file: std::fs::File;
        md_file = std::fs::File::open(in_out_files.0).expect("Error opening md file");
        md_file.read_to_string(&mut md_content).expect("Error during reading");
        html_file = std::fs::File::create(in_out_files.1).expect("Error creating html file");
    }
    
    let mut writer = std::io::BufWriter::new(html_file);
    writer.write_all(convert_md_to_html(md_content).as_bytes()).expect("Error when writing data to file");
}
