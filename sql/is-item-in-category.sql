
SELECT (?1 IN (
      SELECT name
      FROM item_category JOIN category
      ON item_category.cat_id = category.id
      WHERE item_category.item_id = ?2
 )) as "in_category";