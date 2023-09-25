use state_machine::{state_machine, Action, State};

#[derive(Debug, Default)]
struct ParseState {
    is_positive: bool,
    digits_before: Vec<u8>,
    digits_after: Vec<u8>,
    is_exponent_positive: bool,
    digits_exponent: Vec<u8>,
}

#[derive(Debug)]
struct ParseSign;

#[derive(Debug)]
struct ParseDigitsBeforeDot(ParseState);
#[derive(Debug)]
struct ParseDigitsAfterDot(ParseState);
#[derive(Debug)]
struct ParseScientificNotationSign(ParseState);
#[derive(Debug)]
struct ParseScientificNotation(ParseState);
#[derive(Debug)]
struct Finished(ParseState);

#[derive(Debug)]
enum Sign {
    Plus,
    Minus,
}
#[derive(Debug)]
struct Digit(u8);
#[derive(Debug)]
struct Exponential;
#[derive(Debug)]
struct Dot;
#[derive(Debug)]
struct Eos;

state_machine! {
    FloatParser,
    Char,
    ParseSign {
        Sign | Digit => ParseDigitsBeforeDot
    },

    ParseDigitsBeforeDot {
        Digit => ParseDigitsBeforeDot,
        Dot | Exponential => ParseDigitsAfterDot,
        Eos => Finished
    },

    ParseDigitsAfterDot {
         Digit | Exponential => ParseDigitsAfterDot,
         Eos => Finished
    },

    ParseScientificNotationSign {Sign => ParseScientificNotation},
    ParseScientificNotation {
        Digit => ParseScientificNotation,
        Eos => Finished
    },
}

impl State<FloatParser, Sign> for ParseSign {
    fn next(self, action: Sign) -> FloatParser {
        let is_positive = matches!(action, Sign::Plus);
        ParseDigitsBeforeDot(ParseState {
            is_positive,
            is_exponent_positive: true,
            ..Default::default()
        })
        .into()
    }
}

impl State<FloatParser, Digit> for ParseSign {
    fn next(self, action: Digit) -> FloatParser {
        ParseDigitsBeforeDot(ParseState {
            is_positive: true,
            is_exponent_positive: true,
            digits_before: vec![action.0],
            ..Default::default()
        })
        .into()
    }
}

impl State<FloatParser, Dot> for ParseDigitsBeforeDot {
    fn next(self, _action: Dot) -> FloatParser {
        ParseDigitsAfterDot(self.0).into()
    }
}

impl State<FloatParser, Exponential> for ParseDigitsBeforeDot {
    fn next(self, _action: Exponential) -> FloatParser {
        ParseScientificNotation(self.0).into()
    }
}

impl State<FloatParser, Digit> for ParseDigitsBeforeDot {
    fn next(mut self, action: Digit) -> FloatParser {
        self.0.digits_before.push(action.0);
        self.into()
    }
}

impl State<FloatParser, Exponential> for ParseDigitsAfterDot {
    fn next(self, _action: Exponential) -> FloatParser {
        ParseScientificNotationSign(self.0).into()
    }
}

impl State<FloatParser, Digit> for ParseDigitsAfterDot {
    fn next(mut self, action: Digit) -> FloatParser {
        self.0.digits_after.push(action.0);
        self.into()
    }
}

impl State<FloatParser, Digit> for ParseScientificNotation {
    fn next(mut self, action: Digit) -> FloatParser {
        self.0.digits_exponent.push(action.0);
        self.into()
    }
}

impl State<FloatParser, Eos> for ParseScientificNotation {
    fn next(self, _action: Eos) -> FloatParser {
        Finished(self.0).into()
    }
}

impl State<FloatParser, Eos> for ParseDigitsBeforeDot {
    fn next(self, _action: Eos) -> FloatParser {
        Finished(self.0).into()
    }
}

impl State<FloatParser, Eos> for ParseDigitsAfterDot {
    fn next(self, _action: Eos) -> FloatParser {
        Finished(self.0).into()
    }
}

impl State<FloatParser, Sign> for ParseScientificNotationSign {
    fn next(mut self, action: Sign) -> FloatParser {
        self.0.is_exponent_positive = matches!(action, Sign::Plus);
        ParseScientificNotation(self.0).into()
    }
}

fn build_from_parsed(f: Finished) -> f64 {
    let mut integer_part = 0u64;
    for d in f.0.digits_before.iter() {
        integer_part = 10 * integer_part + *d as u64;
    }

    let mut decimal_part = 0u64;
    for d in f.0.digits_after.iter() {
        decimal_part = 10 * decimal_part + *d as u64;
    }

    let mut exponent = 0i32;
    for d in f.0.digits_exponent.iter() {
        exponent = 10 * exponent + *d as i32;
    }
    if !f.0.is_exponent_positive {
        exponent *= -1;
    }

    let no_sign_no_exponent =
        integer_part as f64 + (decimal_part as f64) / (10.0f64).powi(f.0.digits_after.len() as i32);
    let sign = if f.0.is_positive { 1.0 } else { -1.0 };

    sign * no_sign_no_exponent * (10.0f64).powi(exponent)
}

fn main() {
    let input = "3.141596";
    let mut state = FloatParser::from(ParseSign);
    for c in input.chars() {
        let a = match c {
            '+' => Sign::Plus.into(),
            '-' => Sign::Minus.into(),
            'e' => Exponential.into(),
            '.' => Dot.into(),
            c @ '0'..='9' => Digit(c as u8 - b'0').into(),
            c => panic!("Unexpected char found in float: {}", c),
        };

        state = match state.next(a) {
            Err((s, a)) => panic!(
                "Unexpected char when parsing float: state = {:#?}, action = {:#?}",
                s, a
            ),
            Ok(s) => s,
        }
    }

    state = match state.next(Eos.into()) {
        Err((s, a)) => panic!(
            "Unexpected char when parsing float: state = {:#?}, action = {:#?}",
            s, a
        ),
        Ok(s) => s,
    };

    let float = if let FloatParser::Finished(parsed) = state {
        build_from_parsed(parsed)
    } else {
        panic!("Not our terminal state");
    };

    println!(
        "Successfully parsed string {} into float with value '{}'",
        input, float
    );
}
