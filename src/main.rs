use std::env;

fn validate_cmd_arguments(args: Vec<String>) -> Vec<String> {
    if (args.len() == 1 && matches![args[0].as_str(), "-h" | "--help"]) || args.is_empty() { 
        println!("Example usages: 4 + 2, 5 - 4 + 2, 55 * 2, 100 / 2. Operations will be done from left to right. Minimum expected args: 3, between every two digits math operator is required");
        std::process::exit(0);
    }
    else { assert!(args.len() >= 3, "Minimal arguments number is 3"); }

    let mut expect_digit: bool = true;

    for arg in &args {
        match arg.as_str() {
            "+" | "-" | "*" | "/" => {
                if expect_digit { panic!("Expected number, got math operation sign"); }
                else { expect_digit = true; }
            }
            a => {
                if !a.parse::<f32>().is_ok() { panic!("Has something else than number or operation: {}", a); }
                else if !expect_digit { panic!("Expected math operation sign, got a number"); }
                else { expect_digit = false; }
            } 
        }
    }

    args
}

fn calculate(args: Vec<String>) -> f32 {
    
    enum MathOperation {
        Add,
        Sub,
        Mul,
        Div,
    }

    
    let mut result: f32 = args[0].parse::<f32>().unwrap();
    let mut op: Option<MathOperation> = None;    

    for arg in args.into_iter().skip(1) {
        if let std::result::Result::Ok(n) = arg.parse::<f32>() {
            match op {
                Some(MathOperation::Add) => result += n,
                Some(MathOperation::Sub) => result -= n,
                Some(MathOperation::Mul) => result *= n,
                Some(MathOperation::Div) => {

                    if n.round() == 0.0 {
                        panic!("Requsted dividing by 0 - it's disaster");
                    }
                    else { result /= n; }
                }
                None => { panic!("Something bad happened in algorithm. Operation should be set"); }
            }

            op = None;
        }
        else {
            debug_assert!(op.is_none(), "Something bad happened in algorithm. Operation should be empty");
            match arg.as_str() {
                "+" => op = Some(MathOperation::Add),
                "-" => op = Some(MathOperation::Sub),
                "*" => op = Some(MathOperation::Mul),
                "/" => op = Some(MathOperation::Div),
                a => { panic!("Found {} where math operation sign should be", a); }
            }
        }
    }

    result
}

fn main() {
    let result = calculate(validate_cmd_arguments(env::args().skip(1).collect()));
    println!("Operation result: {}", result);
}


#[test]
fn test_validate_cmd_args() {
    let bad_examples = vec![
        vec!["1", "-", "3", "a"],
        vec!["1", "2", "3"],
        vec!["+", "1", "2"],
        vec!["1", "2", "-"],
        vec!["1", "/"],
        vec!["1"]
    ];

    for args in bad_examples {
        let result = std::panic::catch_unwind(|| validate_cmd_arguments(args.iter().map(|s| s.to_string()).collect()));
        assert!(result.is_err(), "Case: {:?} should panic", args);
    }

    let good_examples = vec![
        vec!["1", "+","2"],
        vec!["2.5", "-", "4", "+", "13.13"],
        vec!["0", "*", "1"],
        vec!["10", "/", "3"],
        vec!["-h"],
        vec!["--help"],
        vec![]
    ];

    for args in good_examples {
        let result = std::panic::catch_unwind(|| validate_cmd_arguments(args.iter().map(|s| s.to_string()).collect()));
        assert!(result.is_ok(), "Case: {:?} should be ok", args);
    }
}

#[test]
fn test_calculate() {

    let good_examples = vec![
        (0.0, vec!["0", "+", "0"]),
        (0.0, vec!["0", "*", "0"]),
        (0.0, vec!["5", "*", "0"]),
        (0.0, vec!["0", "*", "5"]),
        (0.0, vec!["0", "-", "0"]),
        (24.0, vec!["5", "+", "5", "+", "5", "+", "5", "+", "4"]),
        (48.0, vec!["100", "-", "10", "-", "15", "-", "25", "-", "2"]),
        (140.0, vec!["2", "*", "7", "*", "10"]),
        (5.0, vec!["100", "/", "2", "/", "10"]),
        (30.25, vec!["10", "+", "0.25", "+", "20"]),
        (12.22, vec!["20", "-", "6", "-", "1.78"]),
        (0.75, vec!["0.5", "*", "0.5", "*", "3"]),
        (0.01, vec!["10", "/", "10", "/", "10", "/", "10"]),
        (-43.5, vec!["10", "+", "-50.25", "+", "-3.25"]),
        (-5.001, vec!["3", "-", "4.5", "-", "3.501"]),
        (-0.125, vec!["0.5", "*", "-0.5", "*", "0.5"]),
        (-0.5, vec!["5", "/", "-10"]),
        (1.00625, vec!["10.5", "+", "-15.25", "-", "3.3", "*", "0.5", "/", "-4"]),
        (1532.0, vec!["1532"])
    ];

    for args in good_examples {
        let result = calculate(args.1.iter().map(|s| s.to_string()).collect());
        assert!(result == args.0, "Expected: {}, Got: {}, For: {:?}", args.0, result, args.1);
    }

    let bad_examples = vec![
        vec!["+"],
        vec!["3", "3", "3"],
        vec!["2", "/", "0"]
    ];

    for args in bad_examples {
        let result = std::panic::catch_unwind(|| calculate(args.iter().map(|s| s.to_string()).collect()));
        assert!(result.is_err(), "Case: {:?} should panic", args);
    }
}
