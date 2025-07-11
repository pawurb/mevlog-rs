use eyre::Result;
use sqlx::Row;

#[allow(dead_code)]
#[derive(Debug)]
pub struct DbChain {
    pub id: i64,
    pub name: String,
    pub explorer_url: Option<String>,
    pub currency_symbol: String,
    pub chainlink_oracle: Option<String>,
}

impl DbChain {
    pub async fn exists(id: i64, conn: &sqlx::SqlitePool) -> Result<bool> {
        let exists = sqlx::query("SELECT EXISTS(SELECT 1 FROM chains WHERE id = ?)")
            .bind(id)
            .fetch_one(conn)
            .await?
            .get::<bool, _>(0);

        Ok(exists)
    }

    pub async fn count(conn: &sqlx::SqlitePool) -> Result<i64> {
        let count = sqlx::query("SELECT COUNT(*) FROM chains")
            .fetch_one(conn)
            .await?
            .get::<i64, _>(0);

        Ok(count)
    }

    pub async fn find(id: i64, conn: &sqlx::SqlitePool) -> Result<Option<DbChain>> {
        let result = sqlx::query(
            r#"
            SELECT id, name, explorer_url, currency_symbol, chainlink_oracle 
            FROM chains 
            WHERE id = ? 
            LIMIT 1
            "#,
        )
        .bind(id)
        .fetch_optional(conn)
        .await?;

        match result {
            Some(row) => Ok(Some(DbChain {
                id: row.get(0),
                name: row.get(1),
                explorer_url: row.get(2),
                currency_symbol: row.get(3),
                chainlink_oracle: row.get(4),
            })),
            None => Ok(None),
        }
    }

    pub async fn find_all(conn: &sqlx::SqlitePool) -> Result<Vec<DbChain>> {
        let rows = sqlx::query(
            r#"
            SELECT id, name, explorer_url, currency_symbol, chainlink_oracle 
            FROM chains 
            ORDER BY id
            "#,
        )
        .fetch_all(conn)
        .await?;

        let chains = rows
            .into_iter()
            .map(|row| DbChain {
                id: row.get(0),
                name: row.get(1),
                explorer_url: row.get(2),
                currency_symbol: row.get(3),
                chainlink_oracle: row.get(4),
            })
            .collect();

        Ok(chains)
    }

    pub async fn save(&self, conn: &sqlx::SqlitePool) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO chains (id, name, explorer_url, currency_symbol, chainlink_oracle)
            VALUES (?, ?, ?, ?, ?)
            "#,
        )
        .bind(self.id)
        .bind(&self.name)
        .bind(&self.explorer_url)
        .bind(&self.currency_symbol)
        .bind(&self.chainlink_oracle)
        .execute(conn)
        .await?;

        Ok(())
    }

    pub async fn update(&self, conn: &sqlx::SqlitePool) -> Result<()> {
        sqlx::query(
            r#"
            UPDATE chains 
            SET name = ?, explorer_url = ?, currency_symbol = ?, chainlink_oracle = ?
            WHERE id = ?
            "#,
        )
        .bind(&self.name)
        .bind(&self.explorer_url)
        .bind(&self.currency_symbol)
        .bind(&self.chainlink_oracle)
        .bind(self.id)
        .execute(conn)
        .await?;

        Ok(())
    }

    pub async fn delete(id: i64, conn: &sqlx::SqlitePool) -> Result<()> {
        sqlx::query("DELETE FROM chains WHERE id = ?")
            .bind(id)
            .execute(conn)
            .await?;

        Ok(())
    }
}

#[cfg(test)]
pub mod test {
    use super::*;
    use crate::models::db_event::test::setup_test_db;

    #[tokio::test]
    async fn create_and_get_chain() -> Result<()> {
        let (conn, _cl) = setup_test_db().await;

        let new_chain = DbChain {
            id: 1,
            name: "Ethereum".to_string(),
            explorer_url: Some("https://etherscan.io".to_string()),
            currency_symbol: "ETH".to_string(),
            chainlink_oracle: Some("0x5f4eC3Df9cbd43714FE2740f5E3616155c5b8419".to_string()),
        };

        new_chain.save(&conn).await?;

        let exists = DbChain::exists(1, &conn).await?;
        assert!(exists);

        assert_eq!(DbChain::count(&conn).await?, 1);

        let found_chain = DbChain::find(1, &conn).await?;
        assert!(found_chain.is_some());

        let chain = found_chain.unwrap();
        assert_eq!(chain.id, 1);
        assert_eq!(chain.currency_symbol, "ETH");
        assert_eq!(chain.explorer_url, Some("https://etherscan.io".to_string()));

        Ok(())
    }

    #[tokio::test]
    async fn find_all_chains() -> Result<()> {
        let (conn, _cl) = setup_test_db().await;

        let chain1 = DbChain {
            id: 1,
            name: "Ethereum".to_string(),
            explorer_url: Some("https://etherscan.io".to_string()),
            currency_symbol: "ETH".to_string(),
            chainlink_oracle: None,
        };

        let chain2 = DbChain {
            id: 56,
            name: "BNB Smart Chain".to_string(),
            explorer_url: Some("https://bscscan.com".to_string()),
            currency_symbol: "BNB".to_string(),
            chainlink_oracle: Some("0x0567F2323251f0Aab15c8dFb1967E4e8A7D42aeE".to_string()),
        };

        chain1.save(&conn).await?;
        chain2.save(&conn).await?;

        let chains = DbChain::find_all(&conn).await?;
        assert_eq!(chains.len(), 2);
        assert_eq!(chains[0].id, 1);
        assert_eq!(chains[1].id, 56);

        Ok(())
    }

    #[tokio::test]
    async fn update_chain() -> Result<()> {
        let (conn, _cl) = setup_test_db().await;

        let mut chain = DbChain {
            id: 1,
            name: "Ethereum".to_string(),
            explorer_url: Some("https://etherscan.io".to_string()),
            currency_symbol: "ETH".to_string(),
            chainlink_oracle: None,
        };

        chain.save(&conn).await?;

        chain.chainlink_oracle = Some("0x5f4eC3Df9cbd43714FE2740f5E3616155c5b8419".to_string());
        chain.update(&conn).await?;

        let updated_chain = DbChain::find(1, &conn).await?.unwrap();
        assert_eq!(
            updated_chain.chainlink_oracle,
            Some("0x5f4eC3Df9cbd43714FE2740f5E3616155c5b8419".to_string())
        );

        Ok(())
    }

    #[tokio::test]
    async fn delete_chain() -> Result<()> {
        let (conn, _cl) = setup_test_db().await;

        let chain = DbChain {
            id: 1,
            name: "Ethereum".to_string(),
            explorer_url: Some("https://etherscan.io".to_string()),
            currency_symbol: "ETH".to_string(),
            chainlink_oracle: None,
        };

        chain.save(&conn).await?;
        assert!(DbChain::exists(1, &conn).await?);

        DbChain::delete(1, &conn).await?;
        assert!(!DbChain::exists(1, &conn).await?);

        Ok(())
    }
}
