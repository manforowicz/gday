use std::{
    collections::VecDeque,
    io::{Read, Write},
};

#[test]
fn test_all() {
    let nonce = [5; 7];
    let key = [5; 32];

    let pipe = VecDeque::new();

    let mut stream = crate::EncryptedStream::new(pipe, &key, &nonce);

    let test_data = [
        &b"abc5423gsgdds43"[..],
        &b"def432gfd2354"[..],
        &b"ggdsgdst43646543hi"[..],
        &b"g"[..],
        &b"mgresgdfgno"[..],
        &b"463prs"[..],
        &b"tufdxb5436w"[..],
        &b"y4325tzz"[..],
        &b"a"[..],
        &b"b"[..],
        &b"132ddsagasfa"[..],
        &b"vds dagdsfa"[..],
        &b" dfsafsadf fsa"[..],
        &b"ete243yfdga"[..],
        &b"dbasbalp35"[..],
        &b";kbfdbaj;dsjagp98845"[..],
        &b"bjkdal;f023590qjva"[..],
        &b"balkdlsaj353osdfa.b"[..],
        &b"bfaa;489ajdfakl;db"[..],
        &b"bsafsda;498fasklj"[..],
        &b";adosp0fspag098b"[..],
        &b"10e92fsa"[..],
        &b"9402389054va"[..],
        &b"xcznvm,.zva"[..],
        &b"0-90`=`=.;[.["[..],
        &b"m.xzc[];][./21"[..],
        &b"10-9].k],.;./,aks"[..],
    ];

    for msg in test_data {
        stream.write_all(msg).unwrap();
        stream.flush_write_buf().unwrap();
        let mut buf = vec![0; msg.len()];
        stream.read_exact(&mut buf).unwrap();
        assert_eq!(buf, msg[..]);
    }
}
