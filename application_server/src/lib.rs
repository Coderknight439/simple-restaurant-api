mod handlers;
mod models;

#[cfg(test)]
mod tests {
    use rusqlite::Connection;
    use warp::{Reply, hyper::Body};
    use handlers::{
        create_menu_handler,
        create_table_handler,
        create_order_handler,
        get_order_item_for_table_handler,
        delete_order_item_handler
    };
    use models::{
        Table,
        Menu,
        OrderRequestBody
    };
    use super::*;
    

    // Set up the test database
    fn setup_test_db() -> Connection {
        println!("Initializing the test database...");
        let conn = Connection::open_in_memory().expect("Failed to create test database");
        conn.execute("PRAGMA foreign_keys = ON;", []).expect("Failed to enable foreign key support");
        conn.execute("CREATE TABLE IF NOT EXISTS tables (id INTEGER PRIMARY KEY,code TEXT NOT NULL UNIQUE)",[]).expect("Table table creation failed");
        conn.execute("CREATE TABLE IF NOT EXISTS menus (id INTEGER PRIMARY KEY, name TEXT NOT NULL)",[]).expect("Menu table creation failed");
        conn.execute("CREATE TABLE IF NOT EXISTS orders (id INTEGER PRIMARY KEY, table_id INTEGER NOT NULL, FOREIGN KEY (table_id) REFERENCES tables(id), UNIQUE (table_id))",[]).expect("Order table creation failed");
        conn.execute("CREATE TABLE IF NOT EXISTS order_items (id INTEGER PRIMARY KEY, order_id INTEGER NOT NULL, menu_id INTEGER NOT NULL, cooking_time INTEGER NOT NULL,  quantity INTEGER NOT NULL default 1, FOREIGN KEY (order_id) REFERENCES orders(id), FOREIGN KEY (menu_id) REFERENCES menus(id))",[]).expect("OrderItems table creation failed");
        conn
    }

    // Inserting static table and menu data
    fn setup_static_data(conn: &Connection){
        let values_to_insert = vec!["T-01", "T-02", "T-03"];

        for value in values_to_insert {
            conn.execute("INSERT INTO tables (code) VALUES (?1)", &[value]).expect("Insertion Failed");
        }
        let values_to_insert = vec!["M-01", "M-02", "M-03", "M-04", "M-05"];

        for value in values_to_insert {
            conn.execute("INSERT INTO menus (name) VALUES (?1)", &[value]).expect("Insertion Failed");
        }
                
    }
    
    // Convert warp Response to serde Json Value
    async fn convert_response_to_json(resp:  warp::http::Response<Body>)->serde_json::Value {
        let body_bytes = warp::hyper::body::to_bytes(resp.into_body()).await.unwrap();
        let body_vec = body_bytes.to_vec();
        let body_string = String::from_utf8_lossy(&body_vec);
        let json_value: serde_json::Value = serde_json::from_str(&body_string).unwrap();
        return json_value;
    }

    // Test Case: 01 Menu Creation
    #[tokio::test]
    async fn test_create_menu_handler() {
        let conn = setup_test_db();
        let menu = Menu {
            id: 0,
            name: "Menu-01".to_string(),
        };
        let result = create_menu_handler(conn, menu).await;
        match result {
            Ok(rep)=>{
                let resp = rep.into_response();
                assert_eq!(resp.status(), warp::http::StatusCode::CREATED);
                let json_data = convert_response_to_json(resp).await;
                assert_eq!(json_data["id"].as_i64(), Some(1));
            }
            Err(_)=>{
                panic!("Unhandled Error");
            }
        }
    }

    // Test Case: 02 Table Creation
    #[tokio::test]
    async fn test_create_table_handler() {
        let conn = setup_test_db();
        let table = Table {
            id: 0,
            code: "Table-01".to_string(),
        };
        let result = create_table_handler(conn, table).await;
        match result {
            Ok(rep)=>{
                let resp = rep.into_response();
                assert_eq!(resp.status(), warp::http::StatusCode::CREATED);
                let json_data = convert_response_to_json(resp).await;
                assert_eq!(json_data["id"].as_i64(), Some(1));
            }
            Err(_)=>{
                panic!("Unhandled Error");
            }
        }
    }

    // Test Case: 03 Order creation fail with wrong data
    #[tokio::test]
    async fn test_create_order_handler_wrong_data() {
        let conn = setup_test_db();
        let order = OrderRequestBody {
            table_id: 1,
            menu_ids: vec![1, 2],
        };
        let result = create_order_handler(conn, order).await;
        // Will raise error, since table and menu not found
        match result {
            Ok(rep)=>{
                let resp = rep.into_response();
                assert_eq!(resp.status(), warp::http::StatusCode::INTERNAL_SERVER_ERROR);
                let json_data = convert_response_to_json(resp).await;
                assert_eq!(json_data["error"].as_str(), Some("Error creating order FOREIGN KEY constraint failed"));
            }
            Err(_)=>{
                panic!("Unhandled Error");
            }
        }
    }
    #[tokio::test]
    async fn test_create_order_handler_wrong_data2() {
        let mut conn = setup_test_db();
        setup_static_data(&mut conn);
        let order = OrderRequestBody {
            table_id: 1,
            menu_ids: vec![],
        };
        let result = create_order_handler(conn, order).await;
        // Will fail, since menu_ids empty
        match result {
            Ok(rep)=>{
                let resp = rep.into_response();
                assert_eq!(resp.status(), warp::http::StatusCode::BAD_REQUEST);
                let json_data = convert_response_to_json(resp).await;
                assert_eq!(json_data["error"].as_str(), Some("Please Add Items"));
            }
            Err(_)=>{
                panic!("Unhandled Error");
            }
        }
    }

    // Test Case: 04 Order creation with correct data
    #[tokio::test]
    async fn test_create_order_handler_correct_data(){
        let conn = setup_test_db();
        setup_static_data(&conn);
        let order = OrderRequestBody {
            table_id: 1,
            menu_ids: vec![1, 2],
        };

        let result = create_order_handler(conn, order).await;
        // Will create a new order for table_id 1 and menu 1, 2
        match result {
            Ok(rep)=>{
                let resp = rep.into_response();
                assert_eq!(resp.status(), warp::http::StatusCode::CREATED);
                let json_data = convert_response_to_json(resp).await;
                assert_eq!(json_data["id"].as_i64(), Some(1));
            }
            Err(_)=>{
                panic!("Unhandled Error");
            }
        }
    
    }

    // Test Case: 05 Remove Item From a Table
    #[tokio::test]
    async fn test_remove_item_from_table_handler(){
        let mut conn = setup_test_db();
        setup_static_data(&conn);
        // Start a transaction for creating order and order items
        let tx = conn.transaction().expect("Transaction Ceation Failed");

        // Insert into the orders table
        tx.execute(
            "INSERT INTO orders (table_id) VALUES (?1)",
            [1],
        ).expect("Order Creation Failed");

        // Get the last inserted order_id
        let order_id = tx.last_insert_rowid();

        // Insert into the order_items table using the obtained order_id
        tx.execute(
            "INSERT INTO order_items (order_id, menu_id, cooking_time) VALUES (?1, ?2, ?3)",
            [order_id, 1, 6],
        ).expect("OrderItems creation failed");

        tx.execute(
            "INSERT INTO order_items (order_id, menu_id, cooking_time) VALUES (?1, ?2, ?3)",
            [order_id, 2, 7],
        ).expect("OrderItems creation failed");

        // Commit the transaction
        tx.commit().expect("Commit Failed");
        let result = delete_order_item_handler(conn, 1, 2).await;
        // Will remove menu 2 from the order, menu 1 will be still there
        match result {
            Ok(rep)=>{
                let resp = rep.into_response();
                assert_eq!(resp.status(), warp::http::StatusCode::OK);
                let json_data = convert_response_to_json(resp).await;
                assert_eq!(json_data["success"].as_str(), Some("Menu deleted successfully"));
            }
            Err(_)=>{
                panic!("Unhandled Error");
            }
        }
    
    }

    // Test Case: 06 Removing all item from a order will delete the order
    #[tokio::test]
    async fn test_all_order_item_remove_handler(){
        let mut conn = setup_test_db();
        setup_static_data(&conn);
        // Start a transaction for creating order and order items
        let tx = conn.transaction().expect("Transaction Ceation Failed");

        // Insert into the orders table
        tx.execute(
            "INSERT INTO orders (table_id) VALUES (?1)",
            [1],
        ).expect("Order Creation Failed");

        // Get the last inserted order_id
        let order_id = tx.last_insert_rowid();

        // Insert into the order_items table using the obtained order_id
        tx.execute(
            "INSERT INTO order_items (order_id, menu_id, cooking_time) VALUES (?1, ?2, ?3)",
            [order_id, 1, 6],
        ).expect("OrderItems creation failed");

        // Commit the transaction
        tx.commit().expect("Commit Failed");
        let result = delete_order_item_handler(conn, 1, 1).await;
        // Will remove menu 1 from the order, and since no item i order, order will be deleted
        match result {
            Ok(rep)=>{
                let resp = rep.into_response();
                assert_eq!(resp.status(), warp::http::StatusCode::OK);
                let json_data = convert_response_to_json(resp).await;
                assert_eq!(json_data["success"].as_str(), Some("Menu deleted successfully and order deleted"));
            }
            Err(_)=>{
                panic!("Unhandled Error");
            }
        }
    
    }

    // Test Case: 07 Removing item having quantity more than 1 will reduce the quantity of the item
    #[tokio::test]
    async fn test_order_item_quantity_reduce_handler(){
        let mut conn = setup_test_db();
        setup_static_data(&conn);
        // Start a transaction for creating order and order items
        let tx = conn.transaction().expect("Transaction Ceation Failed");

        // Insert into the orders table
        tx.execute(
            "INSERT INTO orders (table_id) VALUES (?1)",
            [1],
        ).expect("Order Creation Failed");

        // Get the last inserted order_id
        let order_id = tx.last_insert_rowid();

        // Insert into the order_items table using the obtained order_id
        tx.execute(
            "INSERT INTO order_items (order_id, menu_id, cooking_time, quantity) VALUES (?1, ?2, ?3, ?4)",
            [order_id, 1, 6, 2],
        ).expect("OrderItems creation failed");

        // Commit the transaction
        tx.commit().expect("Commit Failed");
        let result = delete_order_item_handler(conn, 1, 1).await;
        // Will update the quantity of menu 1
        match result {
            Ok(rep)=>{
                let resp = rep.into_response();
                assert_eq!(resp.status(), warp::http::StatusCode::OK);
                let json_data = convert_response_to_json(resp).await;
                assert_eq!(json_data["success"].as_str(), Some("Menu quantity updated successfully"));
            }
            Err(_)=>{
                panic!("Unhandled Error");
            }
        }
    
    }

     // Test Case: 08 Get Specific Item from a Table
    #[tokio::test]
    async fn test_get_item_from_table_handler(){
        let mut conn = setup_test_db();
        setup_static_data(&conn);
        // Start a transaction for creating order and order items
        let tx = conn.transaction().expect("Transaction Ceation Failed");

        // Insert into the orders table
        tx.execute(
            "INSERT INTO orders (table_id) VALUES (?1)",
            [1],
        ).expect("Order Creation Failed");

        // Get the last inserted order_id
        let order_id = tx.last_insert_rowid();

        // Insert into the order_items table using the obtained order_id
        tx.execute(
            "INSERT INTO order_items (order_id, menu_id, cooking_time) VALUES (?1, ?2, ?3)",
            [order_id, 1, 6],
        ).expect("OrderItems creation failed");

        tx.execute(
            "INSERT INTO order_items (order_id, menu_id, cooking_time) VALUES (?1, ?2, ?3)",
            [order_id, 2, 7],
        ).expect("OrderItems creation failed");

        // Commit the transaction
        tx.commit().expect("Commit Failed");

        let result = get_order_item_for_table_handler(conn, 1, 2).await;
        // Will retrieve menu 2 from the table
        match result {
            Ok(rep)=>{
                let resp = rep.into_response();
                match resp.status() {
                    // If item found, get item
                    warp::http::StatusCode::OK=>{
                        let json_data = convert_response_to_json(resp).await;
                        assert_eq!(json_data["menu_name"].as_str(), Some("M-02"));
                    },
                    // If item not found raise NotFound
                    warp::http::StatusCode::NOT_FOUND=>{
                        let json_data = convert_response_to_json(resp).await;
                        assert_eq!(json_data["error"].as_str(), Some("No Item Found"));
                    },
                    _ => {}
                }
            }
            Err(_)=>{
                panic!("Unhandled Error");
            }
        }
    
    }
}