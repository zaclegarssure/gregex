use std::io::{self, Write};

fn main() {
    println!("Gregex REPL");
    println!("Type an empty pattern to exit.");

    loop {
        print!("regex> ");
        io::stdout().flush().unwrap();
        let mut pattern = String::new();
        if io::stdin().read_line(&mut pattern).is_err() {
            println!("Error reading pattern.");
            continue;
        }
        let pattern = pattern.trim();
        if pattern.is_empty() {
            break;
        }

        let jitted = match gregex::Regex::pike_vm(pattern) {
            Ok(regex) => regex,
            Err(e) => {
                print!("Error: {}", e);
                continue;
            }
        };

        loop {
            println!("Type exit to go back to the regex prompt.");
            print!("input> ");
            io::stdout().flush().unwrap();
            let mut input = String::new();
            if io::stdin().read_line(&mut input).is_err() {
                println!("Error reading input.");
                continue;
            }
            let input = input.trim();
            if input == "exit" {
                break;
            }
            match jitted.find_captures(input) {
                Some(m) => {
                    println!("Matched: {}", m.group0().slice());
                }
                None => println!("No match."),
            }
        }
    }
}
