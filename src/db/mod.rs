use std::path::Path;

use chrono::{DateTime, Duration, Local, Utc};
use rusqlite::{named_params, Connection, Error as SQLiteError, Result as SQLiteResult, Row};

const DB_VERSION: u32 = 1;

#[derive(Clone)]
pub struct InfoEntry {
    pub business: String,
    pub owners: String,
    pub street: String,
    pub locality: String,
    pub phone: String,
    pub mail: String,
    pub serial_port: String,
    pub printer_model: Option<String>,
}

impl InfoEntry {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        business: String,
        owners: String,
        street: String,
        locality: String,
        phone: String,
        mail: String,
        serial_port: String,
        printer_model: Option<String>,
    ) -> Self {
        Self {
            business,
            owners,
            street,
            locality,
            phone,
            mail,
            serial_port,
            printer_model,
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
            serial_port: String::from("/dev/ttyUSB0"),
            printer_model: Some(String::from("BrotherQL600")),
        }
    }

    fn load(con: &Connection) -> SQLiteResult<Self> {
        con.query_row(
            "SELECT
                business,
                owners,
                street,
                locality,
                phone,
                mail,
                serial_port,
                printer_model
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
                    serial_port: row.get("serial_port")?,
                    printer_model: row.get("printer_model")?,
                })
            },
        )
    }

    fn store_if_missing(&self, con: &Connection) -> SQLiteResult<()> {
        con.execute(
            "INSERT OR IGNORE INTO info (
                _lock,
                version,
                business,
                owners,
                street,
                locality,
                phone,
                mail,
                serial_port,
                printer_model
            ) VALUES (
                :_lock,
                :version,
                :business,
                :owners,
                :street,
                :locality,
                :phone,
                :mail,
                :serial_port,
                :printer_model
            )",
            named_params! {
                ":_lock": 0,
                ":version": DB_VERSION,
                ":business": self.business,
                ":owners": self.owners,
                ":street": self.street,
                ":locality": self.locality,
                ":phone": self.phone,
                ":mail": self.mail,
                ":serial_port": self.serial_port,
                ":printer_model": self.printer_model
            },
        )?;

        Ok(())
    }

    fn store(&self, con: &Connection) -> SQLiteResult<()> {
        con.execute(
            "REPLACE INTO info (
                _lock,
                version,
                business,
                owners,
                street,
                locality,
                phone,
                mail,
                serial_port,
                printer_model
            ) VALUES (
                :_lock,
                :version,
                :business,
                :owners,
                :street,
                :locality,
                :phone,
                :mail,
                :serial_port,
                :printer_model
            )",
            named_params! {
                ":_lock": 0,
                "version": DB_VERSION,
                ":business": self.business,
                ":owners": self.owners,
                ":street": self.street,
                ":locality": self.locality,
                ":phone": self.phone,
                ":mail": self.mail,
                ":serial_port": self.serial_port,
                ":printer_model": self.printer_model
            },
        )?;

        Ok(())
    }
}

#[derive(Clone)]
pub struct ProductEntry {
    id: Option<i64>,
    pub name: String,
    pub price_ct: u64,
    pub is_kg_price: bool,
    pub ingredients: String,
    pub additional_info: String,
    pub storage_temp: Option<f64>,
    pub expiration_days: Option<u64>,
}

impl ProductEntry {
    pub fn new(
        name: String,
        price_ct: u64,
        is_kg_price: bool,
        ingredients: String,
        additional_info: String,
        storage_temp: Option<f64>,
        expiration_days: Option<u64>,
    ) -> Self {
        Self {
            id: None,
            name,
            price_ct,
            is_kg_price,
            ingredients,
            additional_info,
            storage_temp,
            expiration_days,
        }
    }

    pub fn storage_temp_formatted(&self) -> Option<String> {
        self.storage_temp.map(|temp| format!("{:.1}°C", temp))
    }

    pub fn expiration_date(&self) -> Option<DateTime<Local>> {
        self.expiration_days
            .map(|days| Local::now() + Duration::days(days as _))
    }

    pub fn expiration_date_formatted(&self) -> Option<String> {
        self.expiration_date()
            .map(|date| date.format("%d.%m.%Y %H:%M:%S").to_string())
    }

    fn load(row: &Row) -> SQLiteResult<Self> {
        Ok(Self {
            id: Some(row.get("id")?),
            name: row.get("name")?,
            price_ct: row.get("price_ct")?,
            is_kg_price: row.get("is_kg_price")?,
            ingredients: row.get("ingredients")?,
            additional_info: row.get("additional_info")?,
            storage_temp: row.get("storage_temp")?,
            expiration_days: row.get("expiration_days")?,
        })
    }

    fn load_all(con: &Connection, products: &mut Vec<Self>) -> SQLiteResult<()> {
        let mut stmt = con.prepare(
            "SELECT
                id,
                name,
                price_ct,
                is_kg_price,
                ingredients,
                additional_info,
                storage_temp,
                expiration_days
            FROM products",
        )?;

        products.clear();

        for product in stmt.query_map((), Self::load)? {
            products.push(product?);
        }

        products.sort_by(|p0, p1| p0.name.cmp(&p1.name));

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
                    price_ct,
                    is_kg_price,
                    ingredients,
                    additional_info,
                    storage_temp,
                    expiration_days
                ) VALUES (
                    :id,
                    :name,
                    :price_ct,
                    :is_kg_price,
                    :ingredients,
                    :additional_info,
                    :storage_temp,
                    :expiration_days
                )",
                named_params! {
                    ":id": id,
                    ":name": self.name,
                    ":price_ct": self.price_ct,
                    ":is_kg_price": self.is_kg_price,
                    ":ingredients": self.ingredients,
                    ":additional_info": self.additional_info,
                    ":storage_temp": self.storage_temp,
                    ":expiration_days": self.expiration_days,
                },
            )?;
        } else {
            // If there is no ID, we perform an insert and retrieve the auto-increment afterwards.
            con.execute(
                "INSERT INTO product (
                    name,
                    price_ct,
                    is_kg_price,
                    ingredients,
                    additional_info,
                    storage_temp,
                    expiration_days
                ) VALUES (
                    :name,
                    :price_ct,
                    :is_kg_price,
                    :ingredients,
                    :additional_info,
                    :storage_temp,
                    :expiration_days
                )",
                named_params! {
                    ":name": self.name,
                    ":price_ct": self.price_ct,
                    ":is_kg_price": self.is_kg_price,
                    ":ingredients": self.ingredients,
                    ":additional_info": self.additional_info,
                    ":storage_temp": self.storage_temp,
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

#[derive(Clone)]
pub struct SaleEntry {
    pub date: DateTime<Utc>,
    pub name: String,
    pub weight_kg: Option<f64>,
    pub price_ct: u64,
}

impl SaleEntry {
    pub fn new(date: DateTime<Utc>, name: String, weight_kg: Option<f64>, price_ct: u64) -> Self {
        Self {
            date,
            name,
            weight_kg,
            price_ct,
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
            price_ct: row.get("price_ct")?,
        })
    }

    pub fn load_all(con: &Connection, sales: &mut Vec<Self>) -> SQLiteResult<()> {
        let mut stmt = con.prepare(
            "SELECT
                date_2822,
                name,
                weight_kg,
                price_ct
            FROM sales",
        )?;

        sales.clear();

        for sale in stmt.query_map((), Self::load)? {
            sales.push(sale?);
        }

        sales.sort_by(|s0, s1| s0.date.cmp(&s1.date));

        Ok(())
    }

    pub fn store(&self, con: &Connection) -> SQLiteResult<()> {
        con.execute(
            "INSERT INTO sales (
                date_2822,
                name,
                weight_kg,
                price_ct
            ) VALUES (
                :date_2822,
                :name,
                :weight_kg,
                :price_ct
            )",
            named_params! {
                ":date_2822": self.date.to_rfc2822(),
                ":name": self.name,
                ":weight_kg": self.weight_kg,
                ":price_ct": self.price_ct,
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
                version INTEGER NOT NULL,
                business TEXT NOT NULL,
                owners TEXT NOT NULL,
                street TEXT NOT NULL,
                locality TEXT NOT NULL,
                phone TEXT NOT NULL,
                mail TEXT NOT NULL,
                serial_port TEXT NOT NULL,
                printer_model TEXT
            )",
            (),
        )?;

        con.execute(
            "CREATE TABLE IF NOT EXISTS products (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                name TEXT NOT NULL,
                price_ct INTEGER NOT NULL,
                is_kg_price INTEGER NOT NULL,
                ingredients TEXT NOT NULL,
                additional_info TEXT NOT NULL,
                storage_temp REAL,
                expiration_days INTEGER
            )",
            (),
        )?;

        con.execute(
            "CREATE TABLE IF NOT EXISTS sales (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                date_2822 TEXT NOT NULL,
                name TEXT NOT NULL,
                weight_kg REAL,
                price_ct INTEGER NOT NULL
            )",
            (),
        )?;

        // Query the DB version.
        match con.query_row("SELECT * FROM info", (), |row| {
            // If there is a row, but no version column, this is version 0.
            Ok(row.get("version").unwrap_or(0))
        }) {
            // Validate the version if there is one.
            Ok(version) => {
                if version != DB_VERSION {
                    panic!(
                        "Version mismatch: expected {DB_VERSION}, got {version}. Please migrate!"
                    );
                }
            }

            Err(err) => {
                // If there is no row yet, this is a fresh DB and we can set our own version.
                if err != SQLiteError::QueryReturnedNoRows {
                    panic!("Failed to query version: {err:?}")
                }
            }
        };

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

    pub fn reload_info(&mut self) -> SQLiteResult<()> {
        self.info = InfoEntry::load(&self.con)?;
        Ok(())
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

    pub fn sales(&self, sales: &mut Vec<SaleEntry>) -> SQLiteResult<()> {
        SaleEntry::load_all(&self.con, sales)?;
        Ok(())
    }

    pub fn add_sale(&self, new_sale: &SaleEntry) -> SQLiteResult<()> {
        new_sale.store(&self.con)?;
        Ok(())
    }
}
