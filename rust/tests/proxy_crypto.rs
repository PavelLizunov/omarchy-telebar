//! Known-answer AES, packet bounds and local listener lifetime. No Telegram I/O.
use telebar_runtime::proxy_crypto::{
    ABRIDGED, Crypto, Ctr, SECURE, Splitter, client_handshake, relay_with_random,
};
#[test]
fn ctr_known_answer_handshake_packet_and_owner_lifetime() {
    let bytes = |s: &str| {
        s.as_bytes()
            .as_chunks::<2>()
            .0
            .iter()
            .map(|p| u8::from_str_radix(std::str::from_utf8(p).unwrap(), 16).unwrap())
            .collect::<Vec<_>>()
    };
    let key = bytes("603deb1015ca71be2b73aef0857d77811f352c073b6108d72d9810a30914dff4");
    let iv = bytes("f0f1f2f3f4f5f6f7f8f9fafbfcfdfeff");
    let plain = bytes("6bc1bee22e409f96e93d7e117393172a");
    let expected = bytes("601ec313775789a5b7a7f504bbf3d228");
    assert_eq!(
        Ctr::new(&key, &iv).unwrap().update(&plain).unwrap(),
        expected
    );
    let mut seed = [0; 64];
    for (i, b) in seed.iter_mut().enumerate() {
        *b = i as u8;
    }
    let relay = relay_with_random(seed, [11, 12], SECURE, -4).unwrap();
    let mut cipher = Ctr::new(&relay[8..40], &relay[40..56]).unwrap();
    let decrypted = cipher.update(&relay).unwrap();
    assert_eq!(&decrypted[56..60], &SECURE);
    assert_eq!(&decrypted[60..62], &(-4i16).to_le_bytes());
    assert!(client_handshake(&seed, &[0; 16]).is_err());
    assert!(client_handshake(&seed[..63], &[0; 16]).is_err());
    let mut out = Ctr::new(&relay[8..40], &relay[40..56]).unwrap();
    out.update(&[0; 64]).unwrap();
    let packet = out.update(&[4, 0, 0, 0, 1, 2, 3, 4]).unwrap();
    let mut splitter = Splitter::new(&relay, SECURE).unwrap();
    assert!(splitter.split(&packet[..2]).unwrap().is_empty());
    assert_eq!(splitter.split(&packet[2..]).unwrap(), [packet]);
    let relay = relay_with_random(seed, [11, 12], ABRIDGED, 2).unwrap();
    let mut cipher = Ctr::new(&relay[8..40], &relay[40..56]).unwrap();
    cipher.update(&[0; 64]).unwrap();
    let packet = cipher.update(&[0, 1, 2, 3]).unwrap();
    assert!(
        Splitter::new(&relay, ABRIDGED)
            .unwrap()
            .split(&packet)
            .is_err()
    );
    let _owner = Crypto::new(&seed, &[0; 16], &relay).unwrap();
    let (port, stop, thread) = telebar_runtime::proxy::fixture_lifetime([0; 16]).unwrap();
    let connection = std::net::TcpStream::connect((std::net::Ipv4Addr::LOCALHOST, port)).unwrap();
    stop.send(true).unwrap();
    assert!(thread.join().unwrap().is_ok());
    drop(connection);
    assert!(std::net::TcpStream::connect((std::net::Ipv4Addr::LOCALHOST, port)).is_err());
}
