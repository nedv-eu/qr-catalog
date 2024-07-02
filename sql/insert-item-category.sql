INSERT INTO item_category (cat_id, item_id)
SELECT category.id, ?1 
FROM category 
WHERE category.name = ?2
;