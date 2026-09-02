use colored::Colorize;

pub fn print_success(msg: &str) {
    println!("  {} {}", "✓".green().bold(), msg);
}

pub fn print_error(msg: &str) {
    println!("  {} {}", "✗".red().bold(), msg);
}

pub fn print_warning(msg: &str) {
    println!("  {} {}", "⚠".yellow().bold(), msg);
}

pub fn print_header() {
    println!("{}", "ANTRA".bold().cyan());
    println!();
}
