rm -rf test test2 && mkdir test && mkdir test2 && touch test/x.txt && touch test/untouched.txt && ln test/x.txt test/y.txt && cargo run -- "test/**/*" test/x.txt test2/z.txt && ls -l test

rm -rf test test2 && mkdir -p test/1/2 && mkdir test2 && touch test/x.txt && touch test/untouched.txt && ln test/x.txt test/y.txt && ln test/x.txt test/1/2/1.txt && cargo run -- "test/**/*" test/x.txt test2/z.txt && ls -l test && ls -l test/1/2

rm -rf test test2 test3 && mkdir -p test test2 test3 && touch test/x.txt && touch test/y.txt && ln test/x.txt test2/a.txt && ln test/y.txt test2/b.txt && cargo run -- "test2/**/*" test/x.txt test/y.txt test3/g && ls -l test3
