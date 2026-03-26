use crate::connection::ConnectionManager;

#[derive(Debug, Clone)]
pub struct ConnectedDatabase {
    pub name: String,
    pub host: String,
    pub port: u16,
    pub dbname: String,
    pub user: String,
}

pub fn list_connected(manager: &mut ConnectionManager) -> Vec<ConnectedDatabase> {
    let mut result: Vec<ConnectedDatabase> = manager
        .iter_mut()
        .map(|(_, conn)| ConnectedDatabase {
            name: conn.config.name.clone(),
            host: conn.config.host.clone(),
            port: conn.config.port,
            dbname: conn.config.dbname.clone(),
            user: conn.config.user.clone(),
        })
        .collect();
    result.sort_by(|a, b| a.name.cmp(&b.name));
    result
}

pub fn format_connected_table(databases: &[ConnectedDatabase]) -> String {
    if databases.is_empty() {
        return "No active connections.\n".to_string();
    }

    let name_w = databases
        .iter()
        .map(|d| d.name.len())
        .max()
        .unwrap_or(4)
        .max(4);
    let host_w = databases
        .iter()
        .map(|d| format!("{}:{}", d.host, d.port).len())
        .max()
        .unwrap_or(4)
        .max(4);
    let db_w = databases
        .iter()
        .map(|d| d.dbname.len())
        .max()
        .unwrap_or(8)
        .max(8);
    let user_w = databases
        .iter()
        .map(|d| d.user.len())
        .max()
        .unwrap_or(4)
        .max(4);

    let mut out = String::new();
    out.push_str(&format!(
        "{:<name_w$}  {:<host_w$}  {:<db_w$}  {:<user_w$}\n",
        "Name", "Host", "Database", "User",
    ));
    out.push_str(&format!(
        "{:-<name_w$}  {:-<host_w$}  {:-<db_w$}  {:-<user_w$}\n",
        "", "", "", "",
    ));
    for d in databases {
        out.push_str(&format!(
            "{:<name_w$}  {:<host_w$}  {:<db_w$}  {:<user_w$}\n",
            d.name,
            format!("{}:{}", d.host, d.port),
            d.dbname,
            d.user,
        ));
    }

    out
}
