mod math;

fn main() {
    let input = "sin(x) + cos(x^3 + 2x + pi) - 1.5 * 2.0 / 3.0";
    let lexer = math::lexer::Lexer::new(input);
    let tokens = lexer.tokenize().unwrap();
    for token in tokens {
        println!("{:?}", token);
    }
}
