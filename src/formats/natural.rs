use std::cmp::Ordering;

pub fn natural_cmp(a: &str, b: &str) -> Ordering {
    let mut ia = a.chars().peekable();
    let mut ib = b.chars().peekable();

    loop {
        match (ia.peek(), ib.peek()) {
            (None, None) => return Ordering::Equal,
            (None, Some(_)) => return Ordering::Less,
            (Some(_), None) => return Ordering::Greater,
            (Some(&ca), Some(&cb)) => {
                if ca.is_ascii_digit() && cb.is_ascii_digit() {
                    let mut sa = String::new();
                    while let Some(&c) = ia.peek() {
                        if c.is_ascii_digit() {
                            sa.push(c);
                            ia.next();
                        } else {
                            break;
                        }
                    }
                    let mut sb = String::new();
                    while let Some(&c) = ib.peek() {
                        if c.is_ascii_digit() {
                            sb.push(c);
                            ib.next();
                        } else {
                            break;
                        }
                    }

                    let ta = sa.trim_start_matches('0');
                    let tb = sb.trim_start_matches('0');
                    match ta.len().cmp(&tb.len()) {
                        Ordering::Equal => match ta.cmp(tb) {
                            Ordering::Equal => {}
                            non_eq => return non_eq,
                        },
                        non_eq => return non_eq,
                    }
                } else {
                    ia.next();
                    ib.next();
                    let la = ca.to_lowercase();
                    let lb = cb.to_lowercase();
                    match la.cmp(lb) {
                        Ordering::Equal => {}
                        non_eq => return non_eq,
                    }
                }
            }
        }
    }
}
