use atoms::Atom;

fn main() {
    let v1 = "Test123";
    let mut v2 = "Test".to_string();
    let v3 = &v1[..2];

    v2.push_str("123");

    let atom1: Atom<str> = Atom::new(v1);
    let atom2: Atom<str> = Atom::new(v2);
    let atom3: Atom<str> = Atom::new(v3);

    println!(
        "1: {atom1:?} ({:?})\n2: {atom2:?} ({:?})\n3: {atom3:?} ({:?})\neq12: {}\neq13: {}",
        &*atom1 as *const str,
        &*atom2 as *const str,
        &*atom3 as *const str,
        atom1 == atom2,
        atom1 == atom3
    );
}
