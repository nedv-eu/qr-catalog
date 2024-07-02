
SELECT (0 == (
  SELECT count(cat_id)
  FROM item_category
  WHERE item_category.item_id = ?1
)) as "no_category";