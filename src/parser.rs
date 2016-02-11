use lexer::{Lexer, TokenType, Token};
use nodes::{Node, SpecificNode};


#[derive(Debug)]
pub struct Parser {
    name: String,
    text: String,
    lexer: Lexer,
    root: Node,
    current_token: usize, // where we are in the parsing of the tokens
}

impl Parser {
    pub fn new(name: &str, text: &str) -> Parser {
        let mut lexer = Lexer::new(name, text);
        lexer.run();

        Parser {
            name: name.to_owned(),
            text: text.to_owned(),
            root: Node::new(0, SpecificNode::List(vec![])),
            lexer: lexer,
            current_token: 0
        }
    }

    // Main loop of the parser, stops when there are no token left
    pub fn parse(&mut self) {
        loop {
            let node = match self.parse_next() {
                Some(n) => n,
                None => break
            };

            self.root.push(node);
        }
    }

    // Look at the next token
    fn peek(&self) -> Token {
        self.lexer.tokens.get(self.current_token).unwrap().clone()
    }

    // Look at the next token that isn't space
    fn peek_non_space(&mut self) -> Token {
        let mut token = self.next();
        loop {
            if token.kind != TokenType::Space {
                break;
            }
            token = self.next();
        }
        // Only rewind once (see once i have tests)
        self.current_token -= 1;

        token
    }

    // Get the next token
    fn next(&mut self) -> Token {
        let token = self.peek();
        self.current_token += 1;

        token
    }

    // Get the next token that isn't space
    fn next_non_space(&mut self) -> Token {
        let mut token = self.next();
        loop {
            if token.kind != TokenType::Space {
                break;
            }
            token = self.next();
        }

        token
    }

    // Panics if the expected token isn't found
    fn expect(&mut self, kind: TokenType) -> Token {
        let token = self.peek_non_space();
        if token.kind != kind {
            panic!("Unexpected token: {:?}", token);
        }

        self.next_non_space()
    }

    // All the different "states" the parser can be in: either in a block, in text
    // or in a tag
    fn parse_next(&mut self) -> Option<Box<Node>> {
        loop {
            match self.peek().kind {
                TokenType::TagStart => (),  // TODO
                TokenType::VariableStart => return self.parse_variable_block(),
                TokenType::Text => return self.parse_text(),
                _ => break
            };
        }

        None
    }

    // Parse some html text
    fn parse_text(&mut self) -> Option<Box<Node>> {
        let token = self.next();
        Some(Box::new(Node::new(token.position, SpecificNode::Text(token.value))))
    }

    // Parse the content of a  {{ }} block
    fn parse_variable_block(&mut self) -> Option<Box<Node>> {
        let token = self.expect(TokenType::VariableStart);
        let contained = self.parse_whole_expression(None, TokenType::VariableEnd);
        let node = Node::new(token.position, SpecificNode::VariableBlock(contained.unwrap()));
        self.expect(TokenType::VariableEnd);

        Some(Box::new(node))
    }

    // Parse a block/tag until we get to the terminator
    // Also handles all the precedence
    fn parse_whole_expression(&mut self, stack: Option<Node>, terminator: TokenType) -> Option<Box<Node>> {
        let token = self.peek_non_space();

        let mut node_stack = stack.unwrap_or(Node::new(token.position, SpecificNode::List(vec![])));
        let next = self.parse_single_expression(&terminator).unwrap();
        node_stack.push(next);

        loop {
            let token = self.peek_non_space();
            if token.kind == terminator {
                if node_stack.len() == 0 {
                    panic!("Unexpected terminator");
                }
                return Some(node_stack.pop());
            }

            match token.kind {
                TokenType::Add | TokenType::Substract => {
                    // consume it
                    self.next_non_space();
                    if node_stack.len() == 0 {
                        continue;
                    }

                    let rhs = self.parse_whole_expression(Some(node_stack.clone()), terminator.clone()).unwrap();

                    // Now for + - we need to know if the next token has a higher
                    // precedence (ie * or /)
                    let next_token = self.peek_non_space();
                    if next_token.precedence() > token.precedence() {
                        node_stack.push(rhs);
                        return self.parse_whole_expression(Some(node_stack.clone()), terminator.clone());
                    } else {
                        // Or the next thing has lower precedence and we just
                        // add the node to the stack
                        let lhs = node_stack.pop();
                        let node = if token.kind == TokenType::Add {
                            Node::new(lhs.position, SpecificNode::Addition{lhs: lhs, rhs: rhs})
                        } else {
                            Node::new(lhs.position, SpecificNode::Substraction{lhs: lhs, rhs: rhs})
                        };
                        node_stack.push(Box::new(node));

                    }
                },
                TokenType::Divide | TokenType::Multiply => {
                    // consume the operator
                    self.next_non_space();
                    if node_stack.len() == 0 {
                        panic!("Unexpected division or multiplication");
                    }

                    // * and / have the highest precedence so no need to check
                    // the following operators precedences
                    let rhs = self.parse_single_expression(&terminator).unwrap();
                    let lhs = node_stack.pop();
                    let node = if token.kind == TokenType::Multiply {
                        Node::new(lhs.position, SpecificNode::Multiplication{lhs: lhs, rhs: rhs})
                    } else {
                        Node::new(lhs.position, SpecificNode::Division{lhs: lhs, rhs: rhs})
                    };
                    node_stack.push(Box::new(node));
                },
                _ => panic!("Unexpected token") // TODO: not panic
            }
        }
    }

    // Parses the next non-space token as a simple expression
    // Used when parsing inside a block/tag and we want to get the next value
    fn parse_single_expression(&mut self, terminator: &TokenType) -> Option<Box<Node>> {
        let token = self.peek_non_space();

        if token.kind == *terminator {
            panic!("Unexpected terminator");
        }

        match token.kind {
            TokenType::Identifier => return self.parse_identifier(),
            TokenType::Float | TokenType::Int | TokenType::Bool => return self.parse_literal(),
            TokenType::Add | TokenType::Substract => {
                panic!("wololo");
            }
            _ => panic!("unexpected")
        }

        None
    }

    // Parse an identifier (variable name)
    fn parse_identifier(&mut self) -> Option<Box<Node>> {
        let ident = self.next_non_space();
        Some(Box::new(Node::new(ident.position, SpecificNode::Identifier(ident.value))))
    }

    // Parse a bool/int/float
    fn parse_literal(&mut self) -> Option<Box<Node>> {
        let literal = self.next_non_space();

        match literal.kind {
            TokenType::Int => {
                let value = literal.value.parse::<i32>().unwrap();
                return Some(Box::new(Node::new(literal.position, SpecificNode::Int(value))));
            },
            TokenType::Float => {
                let value = literal.value.parse::<f32>().unwrap();
                return Some(Box::new(Node::new(literal.position, SpecificNode::Float(value))));
            },
            TokenType::Bool => {
                let value = if literal.value == "false" { false } else { true };
                return Some(Box::new(Node::new(literal.position, SpecificNode::Bool(value))));
            },
            _ => panic!("unexpected type when parsing literal")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Parser};
    use nodes::{Node, SpecificNode};

    fn compared_expected(expected: Vec<SpecificNode>, got: Vec<Box<Node>>) {
        if expected.len() != got.len() {
            assert!(false);
        }

        for (i, node) in got.iter().enumerate() {
            let expected_node = expected.get(i).unwrap().clone();
            println!("Expected: {:#?}", expected_node);
            println!("Got: {:#?}", node.specific);
            assert_eq!(expected_node, node.specific);
        }
    }

    fn test_parser(input: &str, expected: Vec<SpecificNode>) {
        let mut parser = Parser::new("dummy", input);
        parser.parse();
        let children = parser.root.get_children();
        compared_expected(expected, children)
    }

    #[test]
    fn test_empty() {
        let mut parser = Parser::new("empty", "");
        parser.parse();
        assert_eq!(0, parser.root.len());
    }

    #[test]
    fn test_plain_string() {
        test_parser(
            "Hello world",
            vec![SpecificNode::Text("Hello world".to_owned())]
        );
    }

    #[test]
    fn test_variable_block_and_text() {
        test_parser(
            "{{ greeting }} 世界",
            vec![
                SpecificNode::VariableBlock(
                    Box::new(Node::new(3, SpecificNode::Identifier("greeting".to_owned())))
                ),
                SpecificNode::Text(" 世界".to_owned()),
            ]
        );
    }

    #[test]
    fn test_basic_math() {
        test_parser(
            "{{1+3.14}}{{1-42}}{{1*42}}{{1/42}}{{test+1}}",
            vec![
                SpecificNode::VariableBlock(
                    Box::new(Node::new(2, SpecificNode::Addition {
                        lhs: Box::new(Node::new(2, SpecificNode::Int(1))),
                        rhs: Box::new(Node::new(4, SpecificNode::Float(3.14))),
                    }))
                ),
                SpecificNode::VariableBlock(
                    Box::new(Node::new(12, SpecificNode::Substraction {
                        lhs: Box::new(Node::new(12, SpecificNode::Int(1))),
                        rhs: Box::new(Node::new(14, SpecificNode::Int(42))),
                    }))
                ),
                SpecificNode::VariableBlock(
                    Box::new(Node::new(20, SpecificNode::Multiplication {
                        lhs: Box::new(Node::new(20, SpecificNode::Int(1))),
                        rhs: Box::new(Node::new(22, SpecificNode::Int(42))),
                    }))
                ),
                SpecificNode::VariableBlock(
                    Box::new(Node::new(28, SpecificNode::Division {
                        lhs: Box::new(Node::new(28, SpecificNode::Int(1))),
                        rhs: Box::new(Node::new(30, SpecificNode::Int(42))),
                    }))
                ),
                SpecificNode::VariableBlock(
                    Box::new(Node::new(36, SpecificNode::Addition {
                        lhs: Box::new(Node::new(36, SpecificNode::Identifier("test".to_owned()))),
                        rhs: Box::new(Node::new(41, SpecificNode::Int(1))),
                    }))
                ),
            ]
        );
    }

    #[test]
    fn test_math_precedence_simple() {
        test_parser(
            "{{ 1 / 2 + 1 }}",
            vec![
                SpecificNode::VariableBlock(
                    Box::new(Node::new(3, SpecificNode::Addition {
                        lhs: Box::new(Node::new(3, SpecificNode::Division {
                            lhs: Box::new(Node::new(3, SpecificNode::Int(1))),
                            rhs: Box::new(Node::new(7, SpecificNode::Int(2))),
                        })),
                        rhs: Box::new(Node::new(11, SpecificNode::Int(1)))
                    }))
                ),
            ]
        );
    }

    #[test]
    fn test_math_precedence_complex() {
        test_parser(
            "{{ 1 / 2 + 3 * 2 + 42 }}", // should be  ((1/2) + (3*2)) + 42
            vec![
                SpecificNode::VariableBlock(
                    Box::new(Node::new(3, SpecificNode::Addition {
                        lhs: Box::new(Node::new(3, SpecificNode::Division {
                            lhs: Box::new(Node::new(3, SpecificNode::Int(1))),
                            rhs: Box::new(Node::new(7, SpecificNode::Int(2))),
                        })),
                        rhs: Box::new(Node::new(11, SpecificNode::Addition {
                            lhs: Box::new(Node::new(11, SpecificNode::Multiplication {
                                lhs: Box::new(Node::new(11, SpecificNode::Int(3))),
                                rhs: Box::new(Node::new(15, SpecificNode::Int(2)))
                            })),
                            rhs: Box::new(Node::new(19, SpecificNode::Int(42)))
                        }))
                    }))
                ),
            ]
        );
    }
}
