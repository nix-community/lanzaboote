use std::{
    convert::Infallible,
    io::{stdout, Cursor, Write},
};

use cpio::NewcReader;
use pio::writer::Cpio;

/*
 * This test is not used in practice,
 * because this is a interactive debugging test.
 * Use it as a model to investigate issues.
 *
 * #[test]
fn visual_diagnose() {
    let mut cpio = Cpio::<Infallible>::new();
    let contents = vec![0xAA; 10];
    let one_size = cpio.pack_one("test.txt", &contents, "", 0o000)
        .expect("Failed to pack a file at the root directory");
    let trailer_size = cpio.pack_trailer()
        .expect("Failed to pack the trailer of the CPIO archive");

    let data = cpio.into_inner();
    stdout().write_all(data.as_slice().escape_ascii().collect::<Vec<u8>>().as_ref())
        .expect("Failed to write the CPIO textual representation");
    print!("\n");

    let reader = NewcReader::new(Cursor::new(data)).expect("Failed to read the first entry");
    let entry = reader.entry();
    println!("entry: {}", entry.name());
    assert_eq!(entry.name(), "/test.txt");
    let reader = NewcReader::new(reader.finish().expect("To finish reading")).expect("Failed to read the trailer");
    let entry = reader.entry();
    println!("entry: {}", entry.name());
}
*/

#[test]
fn alignment() {
    let mut cpio = Cpio::<Infallible>::new();
    let contents = vec![0xAA; 10];
    let one_size = cpio
        .pack_one("test.txt", &contents, "", 0o000)
        .expect("Failed to pack a file at the root directory");
    let trailer_size = cpio
        .pack_trailer()
        .expect("Failed to pack the trailer of the CPIO archive");

    assert!(
        cpio.into_inner().len() % 4 == 0,
        "CPIO is not aligned on a 4 bytes boundary!"
    );
}

#[test]
fn write_read_prefix() {
    let mut cpio = Cpio::<Infallible>::new();
    let contents = vec![0xAA; 10];
    cpio.pack_prefix("a/b/c/d/e/f", 0o600)
        .expect("Failed to pack prefixes of a directory, including itself");

    let data = cpio.into_inner();
    stdout()
        .write_all(data.as_slice().escape_ascii().collect::<Vec<u8>>().as_ref())
        .expect("Failed to write the CPIO textual representation");
    print!("\n");

    let reader = NewcReader::new(Cursor::new(data)).expect("Failed to read the first entry");
    let entry = reader.entry();
    println!("entry: {}", entry.name());
    assert_eq!(entry.name(), "/a");
    let reader = NewcReader::new(reader.finish().expect("To finish reading"))
        .expect("Failed to read the trailer");
    let entry = reader.entry();
    println!("entry: {}", "/a/b");
}

#[test]
fn write_read_basic() {
    let mut cpio = Cpio::<Infallible>::new();
    let contents = vec![0xAA; 10];
    let one_size = cpio
        .pack_one("test.txt", &contents, "", 0o000)
        .expect("Failed to pack a file at the root directory");
    let trailer_size = cpio
        .pack_trailer()
        .expect("Failed to pack the trailer of the CPIO archive");

    assert!(
        cpio.into_inner().len() % 4 == 0,
        "CPIO is not aligned on a 4 bytes boundary!"
    );
}
