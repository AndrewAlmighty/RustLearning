use std::io::BufRead;

type ToSolve = bool;    // if false, don't change value
type BoardType = [[(u8, ToSolve);9];9];

fn parse_arguments(mut args: Vec<String>) -> std::path::PathBuf {
    let print_help = || {
        println!(r"
        Usage: -i <file>.
        File should contains sudoku board 9 x 9, 0 should be in place where number should be find out. Example:
        5 3 0 0 7 0 0 0 0  
        6 0 0 1 9 5 0 0 0  
        0 9 8 0 0 0 0 6 0  
        8 0 0 0 6 0 0 0 3  
        4 0 0 8 0 3 0 0 1  
        7 0 0 0 2 0 0 0 6  
        0 6 0 0 0 0 2 8 0  
        0 0 0 4 1 9 0 0 5  
        0 0 0 0 8 0 0 7 9 
        ");

        std::process::exit(0);
    };

    if args.is_empty() || args.len() != 2 || args[0].as_str() != "-i" {
        print_help();
    }

    args.remove(1).into()
}

fn print_board(board: &BoardType, show_to_solve: bool) {
    let mut board_as_string = String::new();

    for r in 0..9 {
        for c in 0..9 {
            if !show_to_solve && board[r][c].1 {
                board_as_string.push(' ');
            }
            else {
                board_as_string.push((board[r][c].0 + b'0') as char);
            }

            if c == 2 || c == 5 {
                board_as_string.push_str(" | ");
            }
            else {
                board_as_string.push(' ');
            }
        }
        
        if r == 2 || r == 5 {
            board_as_string.push_str("\n- - - - - - - - - - -\n");
        }
        else {
            board_as_string.push('\n');
        }
    }

    println!("{}", board_as_string);
}

fn extract_board_from_file(p: std::path::PathBuf) -> BoardType {
    let f = std::fs::File::open(p).expect("Unable to open file");
    let reader = std::io::BufReader::new(f);

    let mut board: BoardType = [[(0, false);9]; 9];
    let mut r = 0usize;
    let mut c = 0usize;

    for line in reader.lines() {
        assert!(c < 9);
        for ch in line.expect("Unable to extract line from file").trim().chars() {
            assert!(r <9);
            if ch == ' ' {
                continue;
            }

            let value = ch.to_digit(10).expect("Convertion to digit failed") as u8;
            board[r][c] = (value, if value == 0 { true } else { false });
            c = c + 1;
        }
        r = r + 1;
        c = 0;
    }

    println!("Given sudoku:\n");
    print_board(&board, false);

    board
}

fn solve_sudoku(mut board: &mut BoardType) -> Result<(), ()> {
    let create_small_board = |r:usize, c: usize| -> [(u8, u8); 9] {
        let mut i:usize = 0;
        let mut arr:[(u8, u8); 9] = [(0,0); 9];

        for row in r..r+3 {
            for col in c..c+3 {
                assert!(c < 9);
                arr[i] = (row as u8, col as u8);
                i = i +1;
            }
        }

        arr
    };

    let squares: [[(u8, u8); 9]; 9] = [
        create_small_board(0, 0),
        create_small_board(0, 3),
        create_small_board(0, 6),
        create_small_board(3, 0),
        create_small_board(3, 3),
        create_small_board(3, 6),
        create_small_board(6, 0),
        create_small_board(6, 3),
        create_small_board(6, 6)
    ];

    fn solving_process(mut board:&mut BoardType, squares:&[[(u8, u8); 9]; 9], r:usize, c:usize) -> bool {

        let mut valid = false;
        let mut proposed_value = 1u8;
        let field_is_to_set = board[r][c].1;

        while !valid {

            let mut digit_unique = true;

            if field_is_to_set {
                loop {
                    digit_unique = true;
                    if proposed_value >= 10 { 
                        return false;
                    }
                    for tmp_c in 0..9 {
                        if board[r][tmp_c].0 == proposed_value {
                            digit_unique = false;
                            break;
                        }
                    }
                    if digit_unique {
                        for tmp_r in 0..9 {
                            if board[tmp_r][c].0 == proposed_value {
                                digit_unique = false;
                                break;
                            }
                        }
                    }
                    if digit_unique {
                        let idx: usize = 
                        if (0..3).contains(&r) && (0..3).contains(&c) { 0 }
                        else if (0..3).contains(&r) && (3..6).contains(&c) { 1 }
                        else if (0..3).contains(&r) && (6..9).contains(&c) { 2 }
                        else if (3..6).contains(&r) && (0..3).contains(&c) { 3 }
                        else if (3..6).contains(&r) && (3..6).contains(&c) { 4 }
                        else if (3..6).contains(&r) && (6..9).contains(&c) { 5 }
                        else if (6..9).contains(&r) && (0..3).contains(&c) { 6 }
                        else if (6..9).contains(&r) && (3..6).contains(&c) { 7 }
                        else { 8 };
                        for i in 0..9 {
                            if board[squares[idx][i].0 as usize][squares[idx][i].1 as usize].0 == proposed_value {
                                digit_unique = false;
                                break;
                            }
                        }
                    }
                    if digit_unique {
                        board[r][c].0 = proposed_value;
                        break;
                    }
                    else { proposed_value = proposed_value + 1; }
                }
            }

            let next_c = if c >= 8 { 0 } else { c + 1 };
            let next_r = if next_c == 0 { r + 1 } else { r };
            if next_r < 9 {
                valid = solving_process(&mut board, &squares, next_r, next_c);

                if !valid {
                    if field_is_to_set {
                        board[r][c].0 = 0;
                        proposed_value = proposed_value + 1;
                    }
                    else { return false; }
                }
            } else {
                if digit_unique { valid = true; }
                else { return false; }
            }
        }

        valid
    }
    
    if solving_process(&mut board, &squares, 0, 0) { Ok(()) }
    else { Err(()) }
}

fn main() {
    let mut board = extract_board_from_file(parse_arguments(std::env::args().skip(1).collect()));
    match solve_sudoku(&mut board) {
        Ok(()) => { println!("Solved sudoku:\n"); print_board(&board, true); }
        Err(()) => { println!("Couldn't solve given sudoku"); }
    }
}
