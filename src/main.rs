use std::collections::HashMap;

mod math;

fn main() {
    let source = r#"
        ((a+b)^2 + (a+b)*(c-d) + (c-d)^2) / ((a+b)^2 - (a+b)*(c-d) + (c-d)^2)
        + sqrt((((a+b)^2 + (c-d)^2) * ((a+b)^2 + (a+b)*(c-d) + (c-d)^2)))
        + exp(((a+b)*(c-d)) / (1 + (a+b)^2 + (c-d)^2))
        + log(1 + (a+b)^2 + (c-d)^2, 2) / (1 + ((a+b)*(c-d))^2)
        + abs((a+b)^2 - (c-d)^2) / (1 + (a+b)^2 + (c-d)^2)
    "#;

    let tokens = math::lexer::Lexer::new(source).tokenize().unwrap();

    let graph = math::dag::Parser::new(tokens).parse().unwrap();

    let mut variables = HashMap::new();
    variables.insert("a".to_string(), 0.1);
    variables.insert("b".to_string(), 0.2);
    variables.insert("c".to_string(), 0.5);
    variables.insert("d".to_string(), 0.1);

    let result = graph.execute(&variables).unwrap();

    println!("Result: {}", result);
    println!("DAG nodes: {:?}", graph);
}
