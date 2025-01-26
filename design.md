Features I want to support:
- Diagnostics (options to show reference, content of first verse, or content of all verses)
- Hover (customizable context)
- Auto-completion
- Go to definition

With all of these, I want a custom formatter

```
### Ephesians 1:1-4,5-7,2:3-4

[1:1] Paul, an apostle of Christ Jesus by the will of God, To the saints who are in Ephesus, and are faithful in Christ Jesus:
[1:2] Grace to you and peace from God our Father and the Lord Jesus Christ.
[1:3] Blessed be the God and Father of our Lord Jesus Christ, who has blessed us in Christ with every spiritual blessing in the heavenly places,
[1:4] even as he chose us in him before the foundation of the world, that we should be holy and blameless before him. In love

[1:5] he predestined us for adoption to himself as sons through Jesus Christ, according to the purpose of his will,
[1:6] to the praise of his glorious grace, with which he has blessed us in the Beloved.
[1:7] In him we have redemption through his blood, the forgiveness of our trespasses, according to the riches of his grace,

[2:3] among whom we all once lived in the passions of our flesh, carrying out the desires of the body and the mind, and were by nature children of wrath, like the rest of mankind.
[2:4] But God, being rich in mercy, because of the great love with which he loved us,

────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────
```

is represented by

header:
```
### {book} {all_reference_segments}

```
header:
```
### {book} {all_reference_segments}

```
