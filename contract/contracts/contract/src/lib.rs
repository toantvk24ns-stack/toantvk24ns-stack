#![no_std]

use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype, panic_with_error, token, Address, Env,
    String, Symbol,
};

#[derive(Clone)]
#[contracttype]
pub enum DataKey {
    Admin,
    InvoiceCounter,
    Invoice(u64),
}

#[derive(Clone)]
#[contracttype]
pub struct Invoice {
    pub id: u64,
    pub payer: Address,
    pub freelancer: Address,
    pub token: Address,
    pub amount: i128,
    pub memo: String,
    pub paid: bool,
    pub released: bool,
    pub cancelled: bool,
    pub created_at: u64,
    pub paid_at: u64,
}

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum Error {
    AlreadyInitialized = 1,
    NotInitialized = 2,
    Unauthorized = 3,
    InvalidAmount = 4,
    InvoiceNotFound = 5,
    AlreadyPaid = 6,
    NotPaid = 7,
    AlreadyReleased = 8,
    Cancelled = 9,
}

#[contract]
pub struct PayLinkGlobalContract;

#[contractimpl]
impl PayLinkGlobalContract {
    pub fn initialize(env: Env, admin: Address) {
        if env.storage().instance().has(&DataKey::Admin) {
            panic_with_error!(&env, Error::AlreadyInitialized);
        }

        admin.require_auth();
        env.storage().instance().set(&DataKey::Admin, &admin);
        env.storage().instance().set(&DataKey::InvoiceCounter, &0u64);
    }

    pub fn create_invoice(
        env: Env,
        payer: Address,
        freelancer: Address,
        token: Address,
        amount: i128,
        memo: String,
    ) -> u64 {
        let admin = Self::get_admin(&env);
        admin.require_auth();

        if amount <= 0 {
            panic_with_error!(&env, Error::InvalidAmount);
        }

        let mut counter: u64 = env
            .storage()
            .instance()
            .get(&DataKey::InvoiceCounter)
            .unwrap_or(0);

        counter += 1;

        let invoice = Invoice {
            id: counter,
            payer,
            freelancer,
            token,
            amount,
            memo,
            paid: false,
            released: false,
            cancelled: false,
            created_at: env.ledger().timestamp(),
            paid_at: 0,
        };

        env.storage()
            .persistent()
            .set(&DataKey::Invoice(counter), &invoice);
        env.storage()
            .instance()
            .set(&DataKey::InvoiceCounter, &counter);

        counter
    }

    pub fn pay_invoice(env: Env, invoice_id: u64) {
        let mut invoice = Self::get_invoice_or_panic(&env, invoice_id);

        if invoice.cancelled {
            panic_with_error!(&env, Error::Cancelled);
        }
        if invoice.paid {
            panic_with_error!(&env, Error::AlreadyPaid);
        }

        invoice.payer.require_auth();

        let contract_address = env.current_contract_address();
        let token_client = token::TokenClient::new(&env, &invoice.token);

        token_client.transfer(&invoice.payer, &contract_address, &invoice.amount);

        invoice.paid = true;
        invoice.paid_at = env.ledger().timestamp();

        env.storage()
            .persistent()
            .set(&DataKey::Invoice(invoice_id), &invoice);

        env.events().publish(
            (Symbol::new(&env, "invoice_paid"), invoice_id),
            invoice.amount,
        );
    }

    pub fn release_payment(env: Env, invoice_id: u64) {
        let admin = Self::get_admin(&env);
        admin.require_auth();

        let mut invoice = Self::get_invoice_or_panic(&env, invoice_id);

        if invoice.cancelled {
            panic_with_error!(&env, Error::Cancelled);
        }
        if !invoice.paid {
            panic_with_error!(&env, Error::NotPaid);
        }
        if invoice.released {
            panic_with_error!(&env, Error::AlreadyReleased);
        }

        let contract_address = env.current_contract_address();
        let token_client = token::TokenClient::new(&env, &invoice.token);

        token_client.transfer(&contract_address, &invoice.freelancer, &invoice.amount);

        invoice.released = true;

        env.storage()
            .persistent()
            .set(&DataKey::Invoice(invoice_id), &invoice);

        env.events().publish(
            (Symbol::new(&env, "payment_released"), invoice_id),
            invoice.amount,
        );
    }

    pub fn cancel_invoice(env: Env, invoice_id: u64) {
        let admin = Self::get_admin(&env);
        admin.require_auth();

        let mut invoice = Self::get_invoice_or_panic(&env, invoice_id);

        if invoice.paid {
            panic_with_error!(&env, Error::AlreadyPaid);
        }
        if invoice.cancelled {
            panic_with_error!(&env, Error::Cancelled);
        }

        invoice.cancelled = true;

        env.storage()
            .persistent()
            .set(&DataKey::Invoice(invoice_id), &invoice);

        env.events()
            .publish((Symbol::new(&env, "invoice_cancelled"), invoice_id), true);
    }

    pub fn refund_invoice(env: Env, invoice_id: u64) {
        let admin = Self::get_admin(&env);
        admin.require_auth();

        let mut invoice = Self::get_invoice_or_panic(&env, invoice_id);

        if !invoice.paid {
            panic_with_error!(&env, Error::NotPaid);
        }
        if invoice.released {
            panic_with_error!(&env, Error::AlreadyReleased);
        }
        if invoice.cancelled {
            panic_with_error!(&env, Error::Cancelled);
        }

        let contract_address = env.current_contract_address();
        let token_client = token::TokenClient::new(&env, &invoice.token);

        token_client.transfer(&contract_address, &invoice.payer, &invoice.amount);

        invoice.cancelled = true;

        env.storage()
            .persistent()
            .set(&DataKey::Invoice(invoice_id), &invoice);

        env.events().publish(
            (Symbol::new(&env, "invoice_refunded"), invoice_id),
            invoice.amount,
        );
    }

    pub fn get_invoice(env: Env, invoice_id: u64) -> Invoice {
        Self::get_invoice_or_panic(&env, invoice_id)
    }

    pub fn get_admin_address(env: Env) -> Address {
        Self::get_admin(&env)
    }

    pub fn get_invoice_count(env: Env) -> u64 {
        env.storage()
            .instance()
            .get(&DataKey::InvoiceCounter)
            .unwrap_or(0)
    }

    fn get_admin(env: &Env) -> Address {
        env.storage()
            .instance()
            .get(&DataKey::Admin)
            .unwrap_or_else(|| panic_with_error!(env, Error::NotInitialized))
    }

    fn get_invoice_or_panic(env: &Env, invoice_id: u64) -> Invoice {
        env.storage()
            .persistent()
            .get(&DataKey::Invoice(invoice_id))
            .unwrap_or_else(|| panic_with_error!(env, Error::InvoiceNotFound))
    }
}