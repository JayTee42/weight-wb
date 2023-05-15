use std::path::Path;

use chrono::{DateTime, Utc};
use rusqlite::{named_params, Connection, Result as SQLiteResult, Row};

pub struct InfoEntry {
    pub business: String,
    pub owners: String,
    pub street: String,
    pub locality: String,
    pub phone: String,
    pub mail: String,
}

impl InfoEntry {
    pub fn new(
        business: String,
        owners: String,
        street: String,
        locality: String,
        phone: String,
        mail: String,
    ) -> Self {
        Self {
            business,
            owners,
            street,
            locality,
            phone,
            mail,
        }
    }

    fn dummy() -> Self {
        Self {
            business: String::from("<business>"),
            owners: String::from("<owners>"),
            street: String::from("<street>"),
            locality: String::from("<locality>"),
            phone: String::from("<phone>"),
            mail: String::from("<mail>"),
        }
    }

    fn load(con: &Connection) -> SQLiteResult<Self> {
        Ok(con.query_row(
            "SELECT
                business,
                owners,
                street,
                locality,
                phone,
                mail
            FROM info",
            (),
            |row| {
                Ok(Self {
                    business: row.get("business")?,
                    owners: row.get("owners")?,
                    street: row.get("street")?,
                    locality: row.get("locality")?,
                    phone: row.get("phone")?,
                    mail: row.get("mail")?,
                })
            },
        )?)
    }

    fn store_if_missing(&self, con: &Connection) -> SQLiteResult<()> {
        con.execute(
            "INSERT OR IGNORE INTO info (
                _lock,
                business,
                owners,
                street,
                locality,
                phone,
                mail
            ) VALUES (
                :_lock,
                :business,
                :owners,
                :street,
                :locality,
                :phone,
                :mail
            )",
            named_params! {
                ":_lock": 0,
                ":business": self.business,
                ":owners": self.owners,
                ":street": self.street,
                ":locality": self.locality,
                ":phone": self.phone,
                ":mail": self.mail
            },
        )?;

        Ok(())
    }

    fn store(&self, con: &Connection) -> SQLiteResult<()> {
        con.execute(
            "REPLACE INTO info (
                _lock,
                business,
                owners,
                street,
                locality,
                phone,
                mail
            ) VALUES (
                :_lock,
                :business,
                :owners,
                :street,
                :locality,
                :phone,
                :mail
            )",
            named_params! {
                ":_lock": 0,
                ":business": self.business,
                ":owners": self.owners,
                ":street": self.street,
                ":locality": self.locality,
                ":phone": self.phone,
                ":mail": self.mail
            },
        )?;

        Ok(())
    }
}

pub struct ProductEntry {
    id: Option<i64>,
    pub name: String,
    pub ct_per_kg: u64,
    pub ingredients: String,
    pub additional_info: String,
    pub expiration_days: Option<u64>,
}

impl ProductEntry {
    pub fn new(
        name: String,
        ct_per_kg: u64,
        ingredients: String,
        additional_info: String,
        expiration_days: Option<u64>,
    ) -> Self {
        Self {
            id: None,
            name,
            ct_per_kg,
            ingredients,
            additional_info,
            expiration_days,
        }
    }

    fn load(row: &Row) -> SQLiteResult<Self> {
        Ok(Self {
            id: Some(row.get("id")?),
            name: row.get("name")?,
            ct_per_kg: row.get("ct_per_kg")?,
            ingredients: row.get("ingredients")?,
            additional_info: row.get("additional_info")?,
            expiration_days: row.get("expiration_days")?,
        })
    }

    fn load_all(con: &Connection, products: &mut Vec<Self>) -> SQLiteResult<()> {
        let mut stmt = con.prepare(
            "SELECT
                id,
                name,
                ct_per_kg,
                ingredients,
                additional_info,
                expiration_days
            FROM products",
        )?;

        for product in stmt.query_map((), |row| Self::load(row))? {
            products.push(product?);
        }

        Ok(())
    }

    fn store(&mut self, con: &Connection) -> SQLiteResult<()> {
        if let Some(id) = self.id {
            // If we have an ID, the product should be present in the DB.
            // However, it could have been modified from outside.
            // So we force-push our entry via `REPLACE`.
            con.execute(
                "REPLACE INTO product (
                    id,
                    name,
                    ct_per_kg,
                    ingredients,
                    additional_info,
                    expiration_days
                ) VALUES (
                    :id,
                    :name,
                    :ct_per_kg,
                    :ingredients,
                    :additional_info,
                    :expiration_days
                )",
                named_params! {
                    ":id": id,
                    ":name": self.name,
                    ":ct_per_kg": self.ct_per_kg,
                    ":ingredients": self.ingredients,
                    ":additional_info": self.additional_info,
                    ":expiration_days": self.expiration_days,
                },
            )?;
        } else {
            // If there is no ID, we perform an insert and retrieve the auto-increment afterwards.
            con.execute(
                "INSERT INTO product (
                    name,
                    ct_per_kg,
                    ingredients,
                    additional_info,
                    expiration_days
                ) VALUES (
                    :name,
                    :ct_per_kg,
                    :ingredients,
                    :additional_info,
                    :expiration_days
                )",
                named_params! {
                    ":name": self.name,
                    ":ct_per_kg": self.ct_per_kg,
                    ":ingredients": self.ingredients,
                    ":additional_info": self.additional_info,
                    ":expiration_days": self.expiration_days,
                },
            )?;

            self.id = Some(con.last_insert_rowid());
        }

        Ok(())
    }

    fn delete(&self, con: &Connection) -> SQLiteResult<()> {
        let Some(id) = self.id else {
            return Ok(());
        };

        con.execute(
            "DELETE FROM products WHERE id = :id",
            named_params! {":id": id},
        )?;

        Ok(())
    }
}

pub struct SaleEntry {
    pub date: DateTime<Utc>,
    pub name: String,
    pub weight_kg: f64,
    pub ct_per_kg: u64,
}

impl SaleEntry {
    pub fn new(date: DateTime<Utc>, name: String, weight_kg: f64, ct_per_kg: u64) -> Self {
        Self {
            date,
            name,
            weight_kg,
            ct_per_kg,
        }
    }

    pub fn load(row: &Row) -> SQLiteResult<Self> {
        let date_rfc2822: String = row.get("date_2822")?;

        Ok(Self {
            date: DateTime::parse_from_rfc2822(&date_rfc2822)
                .expect("Invalid timestamp format (expected RFC 2822)")
                .into(),
            name: row.get("name")?,
            weight_kg: row.get("weight_kg")?,
            ct_per_kg: row.get("ct_per_kg")?,
        })
    }

    pub fn load_all(&self, con: &Connection, sales: &mut Vec<Self>) -> SQLiteResult<()> {
        let mut stmt = con.prepare(
            "SELECT
                date_2822,
                name,
                weight_kg,
                ct_per_kg
            FROM sales",
        )?;

        for sale in stmt.query_map((), |row| Self::load(row))? {
            sales.push(sale?);
        }

        Ok(())
    }

    pub fn store(&self, con: &Connection) -> SQLiteResult<()> {
        con.execute(
            "INSERT INTO sales (
                date_2822,
                name,
                weight_kg,
                ct_per_kg
            ) VALUES (
                :date,
                :name,
                :weight_kg,
                :ct_per_kg
            )",
            named_params! {
                ":date_2822": self.date.to_rfc2822(),
                ":name": self.name,
                ":weight_kg": self.weight_kg,
                ":ct_per_kg": self.ct_per_kg,
            },
        )?;

        Ok(())
    }
}

pub struct Database {
    con: Connection,
    info: InfoEntry,
    products: Vec<ProductEntry>,
}

impl Database {
    pub fn open_or_create<P: AsRef<Path>>(path: P) -> SQLiteResult<Self> {
        // Open the database.
        let con = Connection::open(path.as_ref())?;

        // Create the tables if they do not exist yet.
        con.execute(
            "CREATE TABLE IF NOT EXISTS info (
                _lock INTEGER NOT NULL PRIMARY KEY,
                business TEXT NOT NULL,
                owners TEXT NOT NULL,
                street TEXT NOT NULL,
                locality TEXT NOT NULL,
                phone TEXT NOT NULL,
                mail TEXT NOT NULL
            )",
            (),
        )?;

        con.execute(
            "CREATE TABLE IF NOT EXISTS products (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                name TEXT NOT NULL,
                ct_per_kg INTEGER NOT NULL,
                ingredients TEXT NOT NULL,
                additional_info TEXT NOT NULL,
                expiration_days INTEGER
            )",
            (),
        )?;

        con.execute(
            "CREATE TABLE IF NOT EXISTS sales (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                date_2822 TEXT NOT NULL,
                name TEXT NOT NULL,
                weight_kg REAL NOT NULL,
                ct_per_kg INTEGER NOT NULL
            )",
            (),
        )?;

        // The `info` table must never be empty.
        // Insert a dummy if necessary before loading it.
        InfoEntry::dummy().store_if_missing(&con)?;
        let info = InfoEntry::load(&con)?;

        // Build the DB and load the products for the first time.
        let mut db = Self {
            con,
            info,
            products: Vec::new(),
        };

        db.reload_products()?;

        Ok(db)
    }

    pub fn info(&self) -> &InfoEntry {
        &self.info
    }

    pub fn update_info<F: FnMut(&mut InfoEntry)>(&mut self, mut f: F) -> SQLiteResult<()> {
        f(&mut self.info);
        self.info.store(&self.con)?;

        Ok(())
    }

    pub fn products(&self) -> &[ProductEntry] {
        &self.products
    }

    pub fn reload_products(&mut self) -> SQLiteResult<()> {
        self.products.clear();
        ProductEntry::load_all(&self.con, &mut self.products)?;

        Ok(())
    }

    pub fn add_product(&mut self, new_product: ProductEntry) -> SQLiteResult<()> {
        self.products.push(new_product);
        self.products.last_mut().unwrap().store(&self.con)?;

        Ok(())
    }

    pub fn update_product<F: FnMut(&mut ProductEntry)>(
        &mut self,
        idx: usize,
        mut f: F,
    ) -> SQLiteResult<()> {
        let product = &mut self.products[idx];

        f(product);
        product.store(&self.con)?;

        Ok(())
    }

    pub fn delete_product(&mut self, idx: usize) -> SQLiteResult<()> {
        let product = self.products.remove(idx);
        product.delete(&self.con)?;

        Ok(())
    }
}
