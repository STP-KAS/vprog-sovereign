//! LUMBRIDGE as one program. Desks are functions in this state, not separate apps.
//! Account 0 is unused. Account 1 is the organiser and cannot take a pot the rules hold.

use crate::merkle::{self, leaf, root, Digest32, Proof, ACCOUNTS};

pub const ORGANISER: u8 = 1;
const PEOPLE: usize = 32;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refuse {
    Short,
    Closed,
    Turn,
    Cell,
    Organiser,
    Early,
    Uncommitted,
    Cap,
    Outsider,
    Jump,
    Price,
    Split,
    Foreign,
    Range,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Exit {
    pub who: u8,
    pub amount: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Token {
    pub name: &'static str,
    pub cap: u64,
    pub supply: u64,
    bal: [u64; PEOPLE],
}

impl Token {
    fn new(name: &'static str, cap: u64) -> Self {
        Self { name, cap, supply: 0, bal: [0; PEOPLE] }
    }

    pub fn balance(&self, who: u8) -> u64 {
        self.bal[who as usize]
    }
}

#[derive(Clone, Debug)]
struct Hunt {
    a: u8,
    b: u8,
    stake: u64,
    pot: u64,
    board: [u8; 9],
    /// 1 means seat A, 2 means seat B, 0 means no match.
    turn: u8,
}

#[derive(Clone, Debug, Default)]
struct Deal {
    buyer: u8,
    seller: u8,
    amount: u64,
    buyer_ok: bool,
    seller_ok: bool,
    holding: u64,
}

#[derive(Clone, Debug)]
struct Vault {
    owner: u8,
    readers: Vec<u8>,
    open_daa: u64,
    note: String,
    bond: u64,
}

#[derive(Clone, Debug)]
struct Circle {
    members: [u8; 4],
    next: usize,
    pot: u64,
    each: u64,
    live: bool,
}

#[derive(Clone, Debug, Default)]
struct Aid {
    pot: u64,
    allow: Vec<u8>,
}

#[derive(Clone, Debug)]
struct Royalty {
    /// Basis points. The shares are sealed at open and sum to 10_000.
    shares: Vec<(u8, u16)>,
    sealed: bool,
}

/// One covenant, one lane, one tree. KAS is the deposit rail. POC and KUSDT are program balances.
#[derive(Clone, Debug)]
pub struct Village {
    covenant: u64,
    daa: u64,
    kas: [u64; PEOPLE],
    locked: [u64; PEOPLE],
    deposits: u128,
    claimed: u128,
    pending: Vec<Exit>,
    committed_exits: Vec<Exit>,
    committed: Option<Digest32>,
    chat_price: u64,
    chat_sealed: bool,
    hunt: Option<Hunt>,
    deal: Deal,
    poc: Token,
    kusdt: Token,
    vault: Option<Vault>,
    circle: Option<Circle>,
    aid: Aid,
    royalty: Royalty,
    queue: Vec<u8>,
}

impl Village {
    pub fn open(covenant: u64, chat_price: u64) -> Self {
        Self {
            covenant,
            daa: 0,
            kas: [0; PEOPLE],
            locked: [0; PEOPLE],
            deposits: 0,
            claimed: 0,
            pending: Vec::new(),
            committed_exits: Vec::new(),
            committed: None,
            chat_price,
            chat_sealed: true,
            hunt: None,
            deal: Deal::default(),
            poc: Token::new("POC", 0),
            kusdt: Token::new("KUSDT", 1_000),
            vault: None,
            circle: None,
            aid: Aid::default(),
            royalty: Royalty { shares: Vec::new(), sealed: false },
            queue: Vec::new(),
        }
    }

    pub fn covenant(&self) -> u64 {
        self.covenant
    }

    pub fn daa(&self) -> u64 {
        self.daa
    }

    pub fn kas(&self, who: u8) -> Result<u64, Refuse> {
        self.person(who)?;
        Ok(self.kas[who as usize])
    }

    pub fn poc(&self, who: u8) -> Result<u64, Refuse> {
        self.person(who)?;
        Ok(self.poc.balance(who))
    }

    pub fn kusdt(&self, who: u8) -> Result<u64, Refuse> {
        self.person(who)?;
        Ok(self.kusdt.balance(who))
    }

    pub fn advance(&mut self, daa: u64) {
        if daa > self.daa {
            self.daa = daa;
        }
    }

    pub fn deposit(&mut self, who: u8, amount: u64) -> Result<(), Refuse> {
        self.person(who)?;
        self.kas[who as usize] = self.kas[who as usize].checked_add(amount).ok_or(Refuse::Range)?;
        self.deposits = self.deposits.checked_add(u128::from(amount)).ok_or(Refuse::Range)?;
        Ok(())
    }

    /// A second covenant cannot be paid from this one. That call is the missing composability.
    pub fn pay_across(&self, other: &Village, _from: u8, _to: u8, _amount: u64) -> Result<(), Refuse> {
        if self.covenant != other.covenant {
            return Err(Refuse::Foreign);
        }
        Err(Refuse::Foreign)
    }

    pub fn withdraw(&mut self, who: u8, amount: u64) -> Result<(), Refuse> {
        self.person(who)?;
        self.take_kas(who, amount)?;
        self.pending.push(Exit { who, amount });
        Ok(())
    }

    pub fn settle(&mut self) -> Digest32 {
        self.committed_exits.append(&mut self.pending);
        let digest = self.digest();
        self.committed = Some(digest);
        digest
    }

    pub fn claim(&mut self, who: u8) -> Result<u64, Refuse> {
        self.person(who)?;
        if self.committed.is_none() {
            return Err(Refuse::Uncommitted);
        }
        let mut paid = 0_u64;
        self.committed_exits.retain(|exit| {
            if exit.who == who {
                paid = paid.saturating_add(exit.amount);
                false
            } else {
                true
            }
        });
        if paid == 0 {
            return Err(Refuse::Short);
        }
        self.claimed = self.claimed.saturating_add(u128::from(paid));
        Ok(paid)
    }

    pub fn digest(&self) -> Digest32 {
        let leaves = self.leaves();
        let balances = root(&leaves);
        let program = merkle::hash_parts(&[
            &self.daa.to_le_bytes(),
            &self.chat_price.to_le_bytes(),
            &self.poc.supply.to_le_bytes(),
            &self.kusdt.supply.to_le_bytes(),
            &self.exit_bytes(),
        ]);
        merkle::node(balances, program)
    }

    pub fn prove_kas(&self, who: u8) -> Result<(u64, Proof), Refuse> {
        self.person(who)?;
        let leaves = self.leaves();
        Ok((self.kas[who as usize], merkle::prove(&leaves, who as usize)))
    }

    pub fn balance_root(&self) -> Digest32 {
        root(&self.leaves())
    }

    pub fn open_hunt(&mut self, a: u8, b: u8, stake: u64) -> Result<(), Refuse> {
        self.person(a)?;
        self.person(b)?;
        if self.hunt.is_some() {
            return Err(Refuse::Closed);
        }
        self.take_kas(a, stake)?;
        self.take_kas(b, stake)?;
        self.locked[a as usize] = self.locked[a as usize].saturating_add(stake);
        self.locked[b as usize] = self.locked[b as usize].saturating_add(stake);
        self.hunt = Some(Hunt {
            a,
            b,
            stake,
            pot: stake.saturating_mul(2),
            board: [0; 9],
            turn: 1,
        });
        Ok(())
    }

    pub fn hunt_move(&mut self, who: u8, cell: u8) -> Result<Option<u8>, Refuse> {
        // None continues the match. Some(None) is a draw. Some(Some(who)) is a win.
        let done = {
            let hunt = self.hunt.as_mut().ok_or(Refuse::Closed)?;
            let seat = if who == hunt.a {
                1
            } else if who == hunt.b {
                2
            } else {
                return Err(Refuse::Turn);
            };
            if hunt.turn != seat {
                return Err(Refuse::Turn);
            }
            let cell = usize::from(cell);
            if cell > 8 || hunt.board[cell] != 0 {
                return Err(Refuse::Cell);
            }
            hunt.board[cell] = seat;
            if let Some(winner_seat) = three(&hunt.board) {
                let winner = if winner_seat == 1 { hunt.a } else { hunt.b };
                Some(Some(winner))
            } else if hunt.board.iter().all(|mark| *mark != 0) {
                Some(None)
            } else {
                hunt.turn = if seat == 1 { 2 } else { 1 };
                None
            }
        };
        match done {
            None => Ok(None),
            Some(winner) => self.pay_hunt(winner),
        }
    }

    pub fn organiser_take_hunt(&mut self) -> Result<(), Refuse> {
        if self.hunt.is_some() {
            Err(Refuse::Organiser)
        } else {
            Err(Refuse::Closed)
        }
    }

    pub fn lock_deal(&mut self, buyer: u8, seller: u8, amount: u64) -> Result<(), Refuse> {
        self.person(buyer)?;
        self.person(seller)?;
        if self.deal.holding != 0 {
            return Err(Refuse::Closed);
        }
        self.take_kas(buyer, amount)?;
        self.locked[buyer as usize] = self.locked[buyer as usize].saturating_add(amount);
        self.deal = Deal { buyer, seller, amount, buyer_ok: false, seller_ok: false, holding: amount };
        Ok(())
    }

    pub fn sign_deal(&mut self, who: u8) -> Result<(), Refuse> {
        if self.deal.holding == 0 {
            return Err(Refuse::Closed);
        }
        if who == self.deal.buyer {
            self.deal.buyer_ok = true;
        } else if who == self.deal.seller {
            self.deal.seller_ok = true;
        } else {
            return Err(Refuse::Organiser);
        }
        if self.deal.buyer_ok && self.deal.seller_ok {
            let amount = self.deal.amount;
            if amount != self.deal.holding {
                return Err(Refuse::Range);
            }
            let buyer = self.deal.buyer;
            let seller = self.deal.seller;
            self.locked[buyer as usize] = self.locked[buyer as usize].saturating_sub(amount);
            self.kas[seller as usize] = self.kas[seller as usize].saturating_add(amount);
            self.deal = Deal::default();
        }
        Ok(())
    }

    pub fn organiser_take_deal(&self) -> Result<(), Refuse> {
        if self.deal.holding == 0 {
            Err(Refuse::Closed)
        } else {
            Err(Refuse::Organiser)
        }
    }

    /// Cap 0 means the mint stays open. A cap above 0 is the most that can be minted.
    pub fn mint_poc(&mut self, who: u8, amount: u64) -> Result<(), Refuse> {
        self.mint_into(&mut self.poc.clone(), who, amount).map(|next| self.poc = next)
    }

    pub fn mint_kusdt(&mut self, who: u8, amount: u64) -> Result<(), Refuse> {
        self.mint_into(&mut self.kusdt.clone(), who, amount).map(|next| self.kusdt = next)
    }

    fn mint_into(&self, token: &mut Token, who: u8, amount: u64) -> Result<Token, Refuse> {
        self.person(who)?;
        if token.cap > 0 && token.supply.saturating_add(amount) > token.cap {
            return Err(Refuse::Cap);
        }
        token.supply = token.supply.checked_add(amount).ok_or(Refuse::Range)?;
        let slot = &mut token.bal[who as usize];
        *slot = slot.checked_add(amount).ok_or(Refuse::Range)?;
        Ok(token.clone())
    }

    /// Both legs move, or neither does. The organiser is not a third balance in the trade.
    pub fn swap_poc_for_kusdt(&mut self, who: u8, poc: u64, kusdt: u64, bank: u8) -> Result<(), Refuse> {
        self.person(who)?;
        self.person(bank)?;
        if who == ORGANISER {
            return Err(Refuse::Organiser);
        }
        if self.poc.balance(who) < poc || self.kusdt.balance(bank) < kusdt {
            return Err(Refuse::Short);
        }
        self.poc.bal[who as usize] -= poc;
        self.poc.bal[bank as usize] = self.poc.bal[bank as usize].saturating_add(poc);
        self.kusdt.bal[bank as usize] -= kusdt;
        self.kusdt.bal[who as usize] = self.kusdt.bal[who as usize].saturating_add(kusdt);
        Ok(())
    }

    pub fn seal_vault(&mut self, owner: u8, readers: Vec<u8>, open_daa: u64, note: String, bond: u64) -> Result<(), Refuse> {
        self.person(owner)?;
        for reader in &readers {
            self.person(*reader)?;
        }
        if self.vault.is_some() {
            return Err(Refuse::Closed);
        }
        self.take_kas(owner, bond)?;
        self.locked[owner as usize] = self.locked[owner as usize].saturating_add(bond);
        self.vault = Some(Vault { owner, readers, open_daa, note, bond });
        Ok(())
    }

    /// Before the open score, a reader gets the hash. The owner can always read the note.
    pub fn read_vault(&self, who: u8) -> Result<VaultView, Refuse> {
        let vault = self.vault.as_ref().ok_or(Refuse::Closed)?;
        if who == vault.owner {
            return Ok(VaultView::Note(vault.note.clone()));
        }
        if !vault.readers.contains(&who) {
            return Err(Refuse::Outsider);
        }
        if self.daa < vault.open_daa {
            let hash = merkle::hash_parts(&[vault.note.as_bytes()]);
            return Ok(VaultView::Sealed(hash));
        }
        Ok(VaultView::Note(vault.note.clone()))
    }

    /// Breaking the seal early pays the bond to the readers and clears the note.
    pub fn break_vault(&mut self, who: u8) -> Result<(), Refuse> {
        let vault = self.vault.clone().ok_or(Refuse::Closed)?;
        if who != vault.owner {
            return Err(Refuse::Organiser);
        }
        if self.daa >= vault.open_daa {
            return Err(Refuse::Early);
        }
        if vault.readers.is_empty() {
            return Err(Refuse::Outsider);
        }
        self.locked[vault.owner as usize] = self.locked[vault.owner as usize].saturating_sub(vault.bond);
        let each = vault.bond / vault.readers.len() as u64;
        let leftover = vault.bond - each * vault.readers.len() as u64;
        for reader in &vault.readers {
            self.kas[*reader as usize] = self.kas[*reader as usize].saturating_add(each);
        }
        self.kas[vault.readers[0] as usize] = self.kas[vault.readers[0] as usize].saturating_add(leftover);
        self.vault = None;
        Ok(())
    }

    pub fn say(&mut self, who: u8) -> Result<(), Refuse> {
        self.person(who)?;
        if !self.chat_sealed {
            return Err(Refuse::Price);
        }
        if self.poc.balance(who) < self.chat_price {
            return Err(Refuse::Short);
        }
        self.poc.bal[who as usize] -= self.chat_price;
        self.poc.supply -= self.chat_price;
        Ok(())
    }

    pub fn reprice_chat(&mut self, who: u8, _price: u64) -> Result<(), Refuse> {
        self.person(who)?;
        if self.chat_sealed {
            Err(Refuse::Price)
        } else {
            Ok(())
        }
    }

    pub fn open_circle(&mut self, members: [u8; 4], each: u64) -> Result<(), Refuse> {
        if self.circle.is_some() {
            return Err(Refuse::Closed);
        }
        for member in members {
            self.person(member)?;
            if self.kas[member as usize] < each {
                return Err(Refuse::Short);
            }
        }
        for member in members {
            self.take_kas(member, each)?;
            self.locked[member as usize] = self.locked[member as usize].saturating_add(each);
        }
        self.circle = Some(Circle {
            members,
            next: 0,
            pot: each.saturating_mul(4),
            each,
            live: true,
        });
        Ok(())
    }

    pub fn circle_pay(&mut self) -> Result<u8, Refuse> {
        let circle = self.circle.as_mut().ok_or(Refuse::Closed)?;
        if !circle.live || circle.pot == 0 {
            return Err(Refuse::Closed);
        }
        let who = circle.members[circle.next];
        let pay = circle.each.min(circle.pot);
        circle.pot -= pay;
        circle.next = (circle.next + 1) % circle.members.len();
        self.locked[who as usize] = self.locked[who as usize].saturating_sub(pay);
        self.kas[who as usize] = self.kas[who as usize].saturating_add(pay);
        if circle.pot == 0 {
            circle.live = false;
        }
        Ok(who)
    }

    pub fn organiser_take_circle(&self) -> Result<(), Refuse> {
        if self.circle.as_ref().map(|c| c.live).unwrap_or(false) {
            Err(Refuse::Organiser)
        } else {
            Err(Refuse::Closed)
        }
    }

    pub fn fund_aid(&mut self, donor: u8, amount: u64, allow: Vec<u8>) -> Result<(), Refuse> {
        self.person(donor)?;
        for who in &allow {
            self.person(*who)?;
        }
        self.take_kas(donor, amount)?;
        self.aid.pot = self.aid.pot.saturating_add(amount);
        self.aid.allow = allow;
        Ok(())
    }

    pub fn pay_aid(&mut self, who: u8, amount: u64) -> Result<(), Refuse> {
        if !self.aid.allow.contains(&who) {
            return Err(Refuse::Outsider);
        }
        if self.aid.pot < amount {
            return Err(Refuse::Short);
        }
        self.aid.pot -= amount;
        self.kas[who as usize] = self.kas[who as usize].saturating_add(amount);
        Ok(())
    }

    pub fn seal_royalty(&mut self, shares: Vec<(u8, u16)>) -> Result<(), Refuse> {
        if self.royalty.sealed {
            return Err(Refuse::Split);
        }
        let sum: u32 = shares.iter().map(|(_, bps)| u32::from(*bps)).sum();
        if sum != 10_000 {
            return Err(Refuse::Split);
        }
        for (who, _) in &shares {
            self.person(*who)?;
        }
        self.royalty = Royalty { shares, sealed: true };
        Ok(())
    }

    pub fn pay_royalty(&mut self, payer: u8, amount: u64) -> Result<(), Refuse> {
        self.person(payer)?;
        if !self.royalty.sealed {
            return Err(Refuse::Split);
        }
        self.take_kas(payer, amount)?;
        let mut sent = 0_u64;
        for (index, (who, bps)) in self.royalty.shares.iter().enumerate() {
            let share = if index + 1 == self.royalty.shares.len() {
                amount - sent
            } else {
                amount * u64::from(*bps) / 10_000
            };
            sent += share;
            self.kas[*who as usize] = self.kas[*who as usize].saturating_add(share);
        }
        Ok(())
    }

    pub fn enqueue(&mut self, who: u8) -> Result<(), Refuse> {
        self.person(who)?;
        self.queue.push(who);
        Ok(())
    }

    pub fn jump_queue(&mut self, who: u8) -> Result<(), Refuse> {
        self.person(who)?;
        let _ = who;
        Err(Refuse::Jump)
    }

    pub fn pop_queue(&mut self) -> Result<u8, Refuse> {
        if self.queue.is_empty() {
            Err(Refuse::Closed)
        } else {
            Ok(self.queue.remove(0))
        }
    }

    pub fn conserved(&self) -> bool {
        let kas: u128 = self.kas.iter().map(|v| u128::from(*v)).sum();
        let locked: u128 = self.locked.iter().map(|v| u128::from(*v)).sum();
        let pending: u128 = self.pending.iter().map(|e| u128::from(e.amount)).sum();
        let committed: u128 = self.committed_exits.iter().map(|e| u128::from(e.amount)).sum();
        // Deal, hunt, circle, and vault bonds sit in `locked`. Aid sits in its own pot.
        kas + locked + pending + committed + u128::from(self.aid.pot) + self.claimed == self.deposits
    }

    fn pay_hunt(&mut self, winner: Option<u8>) -> Result<Option<u8>, Refuse> {
        let hunt = self.hunt.take().ok_or(Refuse::Closed)?;
        self.locked[hunt.a as usize] = self.locked[hunt.a as usize].saturating_sub(hunt.stake);
        self.locked[hunt.b as usize] = self.locked[hunt.b as usize].saturating_sub(hunt.stake);
        match winner {
            Some(who) => {
                self.kas[who as usize] = self.kas[who as usize].saturating_add(hunt.pot);
            }
            None => {
                self.kas[hunt.a as usize] = self.kas[hunt.a as usize].saturating_add(hunt.stake);
                self.kas[hunt.b as usize] = self.kas[hunt.b as usize].saturating_add(hunt.stake);
            }
        }
        Ok(winner)
    }

    fn take_kas(&mut self, who: u8, amount: u64) -> Result<(), Refuse> {
        let slot = &mut self.kas[who as usize];
        if *slot < amount {
            return Err(Refuse::Short);
        }
        *slot -= amount;
        Ok(())
    }

    fn person(&self, who: u8) -> Result<(), Refuse> {
        if who == 0 || usize::from(who) >= PEOPLE {
            Err(Refuse::Range)
        } else {
            Ok(())
        }
    }

    fn leaves(&self) -> [Digest32; ACCOUNTS] {
        let mut leaves = [merkle::empty_leaf(); ACCOUNTS];
        for index in 0..PEOPLE {
            leaves[index] = leaf(self.kas[index], self.locked[index]);
        }
        leaves
    }

    fn exit_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::new();
        for exit in self.pending.iter().chain(self.committed_exits.iter()) {
            bytes.push(exit.who);
            bytes.extend_from_slice(&exit.amount.to_le_bytes());
        }
        bytes
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum VaultView {
    Sealed(Digest32),
    Note(String),
}

fn three(board: &[u8; 9]) -> Option<u8> {
    const LINES: [[usize; 3]; 8] = [
        [0, 1, 2],
        [3, 4, 5],
        [6, 7, 8],
        [0, 3, 6],
        [1, 4, 7],
        [2, 5, 8],
        [0, 4, 8],
        [2, 4, 6],
    ];
    LINES.iter().find_map(|line| {
        let mark = board[line[0]];
        if mark != 0 && mark == board[line[1]] && mark == board[line[2]] {
            Some(mark)
        } else {
            None
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn funded() -> Village {
        let mut town = Village::open(7, 1);
        town.deposit(2, 50).unwrap();
        town.deposit(3, 50).unwrap();
        town.deposit(4, 50).unwrap();
        town.deposit(5, 50).unwrap();
        town.deposit(6, 50).unwrap();
        town
    }

    #[test]
    fn hunt_pays_the_winner_and_the_organiser_cannot_take_the_pot() {
        let mut town = funded();
        town.open_hunt(3, 4, 10).unwrap();
        assert_eq!(town.organiser_take_hunt(), Err(Refuse::Organiser));
        // X X X on the top row, with O answering on the left column.
        assert_eq!(town.hunt_move(3, 0).unwrap(), None);
        assert_eq!(town.hunt_move(4, 3).unwrap(), None);
        assert_eq!(town.hunt_move(3, 1).unwrap(), None);
        assert_eq!(town.hunt_move(4, 4).unwrap(), None);
        assert_eq!(town.hunt_move(3, 2).unwrap(), Some(3));
        assert_eq!(town.kas(3).unwrap(), 60);
        assert_eq!(town.kas(4).unwrap(), 40);
        assert!(town.conserved());
    }

    #[test]
    fn a_draw_splits_the_hunt_pot() {
        let mut town = funded();
        town.open_hunt(3, 4, 5).unwrap();
        let draws = [0_u8, 1, 2, 4, 3, 5, 7, 6, 8];
        let mut last = None;
        for (index, cell) in draws.into_iter().enumerate() {
            let who = if index % 2 == 0 { 3 } else { 4 };
            last = town.hunt_move(who, cell).unwrap();
        }
        assert_eq!(last, None);
        assert_eq!(town.kas(3).unwrap(), 50);
        assert_eq!(town.kas(4).unwrap(), 50);
        assert!(town.conserved());
    }

    #[test]
    fn market_pays_the_seller_only_after_both_sign() {
        let mut town = funded();
        town.lock_deal(3, 4, 15).unwrap();
        assert_eq!(town.organiser_take_deal(), Err(Refuse::Organiser));
        town.sign_deal(3).unwrap();
        assert_eq!(town.kas(4).unwrap(), 50);
        town.sign_deal(4).unwrap();
        assert_eq!(town.kas(3).unwrap(), 35);
        assert_eq!(town.kas(4).unwrap(), 65);
        assert!(town.conserved());
    }

    #[test]
    fn mint_cap_and_open_cap_do_not_touch_kas() {
        let mut town = funded();
        let before = town.kas(3).unwrap();
        town.mint_poc(3, 40).unwrap();
        town.mint_poc(4, 60).unwrap();
        assert_eq!(town.poc(3).unwrap(), 40);
        assert!(town.mint_kusdt(3, 1001).is_err());
        town.mint_kusdt(3, 1000).unwrap();
        assert_eq!(town.mint_kusdt(4, 1), Err(Refuse::Cap));
        assert_eq!(town.kas(3).unwrap(), before);
        assert!(town.conserved());
    }

    #[test]
    fn swap_is_atomic() {
        let mut town = funded();
        town.mint_poc(3, 10).unwrap();
        town.mint_kusdt(2, 7).unwrap();
        assert_eq!(town.swap_poc_for_kusdt(3, 10, 8, 2), Err(Refuse::Short));
        assert_eq!(town.poc(3).unwrap(), 10);
        assert_eq!(town.kusdt(2).unwrap(), 7);
        town.swap_poc_for_kusdt(3, 10, 7, 2).unwrap();
        assert_eq!(town.poc(3).unwrap(), 0);
        assert_eq!(town.poc(2).unwrap(), 10);
        assert_eq!(town.kusdt(3).unwrap(), 7);
        assert_eq!(town.kusdt(2).unwrap(), 0);
    }

    #[test]
    fn vault_hides_the_note_until_the_daa_and_a_break_pays_the_bond() {
        let mut town = funded();
        town.seal_vault(3, vec![4], 100, "night market".into(), 9).unwrap();
        match town.read_vault(4).unwrap() {
            VaultView::Sealed(_) => {}
            VaultView::Note(_) => panic!("early read"),
        }
        assert!(matches!(town.read_vault(3).unwrap(), VaultView::Note(_)));
        town.break_vault(3).unwrap();
        assert_eq!(town.kas(4).unwrap(), 59);
        assert_eq!(town.read_vault(4), Err(Refuse::Closed));
        assert!(town.conserved());
    }

    #[test]
    fn chat_price_is_sealed_and_debits_poc() {
        let mut town = funded();
        town.mint_poc(5, 3).unwrap();
        town.say(5).unwrap();
        assert_eq!(town.poc(5).unwrap(), 2);
        assert_eq!(town.reprice_chat(1, 9), Err(Refuse::Price));
        assert!(town.conserved());
    }

    #[test]
    fn circle_rotates_and_the_organiser_cannot_withdraw_it() {
        let mut town = funded();
        town.open_circle([3, 4, 5, 6], 10).unwrap();
        assert_eq!(town.organiser_take_circle(), Err(Refuse::Organiser));
        assert_eq!(town.circle_pay().unwrap(), 3);
        assert_eq!(town.kas(3).unwrap(), 50);
        assert!(town.conserved());
    }

    #[test]
    fn aid_stays_on_the_list_and_royalty_uses_the_sealed_shares() {
        let mut town = funded();
        town.fund_aid(3, 20, vec![4]).unwrap();
        assert_eq!(town.pay_aid(5, 5), Err(Refuse::Outsider));
        town.pay_aid(4, 5).unwrap();
        assert_eq!(town.kas(4).unwrap(), 55);
        town.seal_royalty(vec![(4, 2500), (5, 7500)]).unwrap();
        assert_eq!(town.seal_royalty(vec![(4, 10_000)]), Err(Refuse::Split));
        town.pay_royalty(6, 8).unwrap();
        assert_eq!(town.kas(4).unwrap(), 57);
        assert_eq!(town.kas(5).unwrap(), 56);
        assert!(town.conserved());
    }

    #[test]
    fn the_queue_pops_the_tail_and_refuses_a_jump() {
        let mut town = funded();
        town.enqueue(4).unwrap();
        town.enqueue(5).unwrap();
        assert_eq!(town.jump_queue(1), Err(Refuse::Jump));
        assert_eq!(town.pop_queue().unwrap(), 4);
        assert_eq!(town.pop_queue().unwrap(), 5);
    }

    #[test]
    fn an_exit_waits_for_settlement_and_then_conserves() {
        let mut town = funded();
        town.withdraw(3, 12).unwrap();
        assert_eq!(town.claim(3), Err(Refuse::Uncommitted));
        let digest = town.settle();
        assert_eq!(town.digest(), digest);
        assert_eq!(town.claim(3).unwrap(), 12);
        assert_eq!(town.kas(3).unwrap(), 38);
        assert!(town.conserved());
    }

    #[test]
    fn two_villages_do_not_pay_each_other() {
        let a = funded();
        let b = Village::open(8, 1);
        let before = a.kas(3).unwrap();
        assert_eq!(a.pay_across(&b, 3, 4, 5), Err(Refuse::Foreign));
        assert_eq!(a.kas(3).unwrap(), before);
        assert_ne!(a.digest(), b.digest());
    }

    #[test]
    fn a_balance_proof_dies_when_another_account_changes() {
        let mut town = funded();
        let before = town.digest();
        let (amount, proof) = town.prove_kas(3).unwrap();
        let root = town.balance_root();
        let leaf = merkle::leaf(amount, 0);
        assert!(merkle::verify(root, 3, leaf, &proof));
        town.deposit(4, 1).unwrap();
        assert_ne!(town.digest(), before);
        assert!(!merkle::verify(town.balance_root(), 3, leaf, &proof));
        let (again, proof) = town.prove_kas(3).unwrap();
        assert_eq!(again, amount);
        assert!(merkle::verify(town.balance_root(), 3, leaf, &proof));
    }
}
