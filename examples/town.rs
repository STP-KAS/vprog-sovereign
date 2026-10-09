//! Walk one LUMBRIDGE lab day. Prints the rules that held. Submits nothing.

use vprog_sovereign::{Refuse, Village, VaultView};

fn main() {
    let mut town = Village::open(1984, 1);
    for who in [2_u8, 3, 4, 5, 6] {
        town.deposit(who, 100).unwrap();
    }

    town.open_hunt(3, 4, 10).unwrap();
    assert_eq!(town.organiser_take_hunt(), Err(Refuse::Organiser));
    town.hunt_move(3, 0).unwrap();
    town.hunt_move(4, 3).unwrap();
    town.hunt_move(3, 1).unwrap();
    town.hunt_move(4, 4).unwrap();
    let winner = town.hunt_move(3, 2).unwrap();
    println!("hunt winner {winner:?} pot stays out of the organiser account");

    town.lock_deal(5, 6, 20).unwrap();
    assert_eq!(town.organiser_take_deal(), Err(Refuse::Organiser));
    town.sign_deal(5).unwrap();
    town.sign_deal(6).unwrap();
    println!("market paid the seller {} KAS", town.kas(6).unwrap());

    town.mint_poc(3, 40).unwrap();
    town.mint_kusdt(2, 25).unwrap();
    town.swap_poc_for_kusdt(3, 10, 7, 2).unwrap();
    println!(
        "swap left the buyer with {} POC and {} KUSDT",
        town.poc(3).unwrap(),
        town.kusdt(3).unwrap()
    );

    town.seal_vault(4, vec![5], 500, "the night list".into(), 4).unwrap();
    assert!(matches!(town.read_vault(5).unwrap(), VaultView::Sealed(_)));
    town.advance(500);
    assert!(matches!(town.read_vault(5).unwrap(), VaultView::Note(_)));
    println!("vault opened at DAA {}", town.daa());

    town.say(3).unwrap();
    assert_eq!(town.reprice_chat(1, 5), Err(Refuse::Price));
    println!("chat still costs the sealed price");

    town.open_circle([3, 4, 5, 6], 5).unwrap();
    assert_eq!(town.organiser_take_circle(), Err(Refuse::Organiser));
    println!("circle paid {}", town.circle_pay().unwrap());

    let other = Village::open(1985, 1);
    assert_eq!(town.pay_across(&other, 3, 4, 1), Err(Refuse::Foreign));
    println!("a second covenant was refused");

    town.withdraw(6, 15).unwrap();
    assert_eq!(town.claim(6), Err(Refuse::Uncommitted));
    let digest = town.settle();
    let paid = town.claim(6).unwrap();
    println!("exit {paid} paid after settlement {digest:02x?}");
    assert!(town.conserved());
    println!("KAS books conserved");
}
