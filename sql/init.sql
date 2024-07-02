CREATE TABLE IF NOT EXISTS category (
     id INTEGER PRIMARY KEY,
     name TEXT UNIQUE NOT NULL
);

CREATE TABLE IF NOT EXISTS item_category (
    item_id INTEGER,
    cat_id INTEGER,
    PRIMARY KEY ( item_id, cat_id),
    FOREIGN KEY(cat_id) REFERENCES category(id)
);

