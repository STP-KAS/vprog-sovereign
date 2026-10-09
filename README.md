# vprog-sovereign

A lab for one sovereign app on the vprogs shape. [DISCLAIMER.md](DISCLAIMER.md).

On 9 October 2026 Maksim Biriukov published [vprogs: a based rollup on Kaspa](https://biryukovmaxim.github.io/vprogs-workshop/). The book links the framework at kaspanet/vprogs `055ae28a` and the tic-tac-toe example at biryukovmaxim/vprog-tictactoe `fe6b0e85`. Those trees are behind the live tips. The book's own line is the one this crate keeps: the rollup fixes the shape, the program picks the rules, and one program does not call another.

## What runs

`Village` is one covenant. People deposit KAS. Hunt, market, wage, mint, loyalty, swap, vault, chat, meter, circle, aid, royalty, a queue, and a Layer-Kaspa offer are functions in that one state. `card` names the operator and prints the stall next to the price: a committed exit cannot be withheld, an uncommitted balance waits if every operator stops, and there is no forced-inclusion path and no escape hatch. An exit is a pending leaf until `settle`, and `claim` pays it only after that. `pay_across` between two covenants returns `Foreign`. The archival peer is unnamed. The image pin is a lab digest, not a guest image id.

POC and KUSDT in this crate are program balances. They are not KAS and they are not KCC-20.

```
cargo test
cargo run --example town
```

`cargo test` is the run. It does not open a socket.

## Map

[MAP.md](MAP.md) is the proposition: Web 4.0, industries, what other chains already do, and which LUMBRIDGE desks would move into one village program.

## Pins this crate does not move

| Object | Tip this desk read on 9 Oct 2026 | What the book cites |
| --- | --- | --- |
| kaspanet/vprogs master | `f9b84a8` | |
| kaspanet/vprogs `release-candidate` | `cc0d54bc` | `055ae28a` (10 commits behind) |
| biryukovmaxim/vprog-tictactoe master | `f93e52fe` (root lock `cc0d54bc`, guest lock `04cfb0ae`) | `fe6b0e85` (13 commits behind) |

The guest lock and the root lock are not one pin. This crate does not submit to https://vprogs-tt.izio.fr/.
