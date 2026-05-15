pub fn pair_close(c: char) -> Option<char> {
    match c {
        '(' => Some(')'),
        '[' => Some(']'),
        '{' => Some('}'),
        '"' => Some('"'),
        _   => None,
    }
}

pub fn is_pair(open: char, close: char) -> bool {
    matches!(
        (open, close),
        ('(', ')') | ('[', ']') | ('{', '}') | ('"', '"')
    )
}

pub fn should_delete_pair(prev: Option<char>, next: Option<char>) -> bool {
    match (prev, next) {
        (Some(p), Some(n)) => is_pair(p, n),
        _                  => false,
    }
}
