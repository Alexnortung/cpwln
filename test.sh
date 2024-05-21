rm -rf test test2 && mkdir test && mkdir test2 && touch test/x.txt && touch test/untouched.txt && ln test/x.txt test/y.txt && cargo run -- "test/**/*" test/x.txt test2/z.txt && ls -l test
