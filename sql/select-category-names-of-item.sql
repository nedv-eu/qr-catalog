
SELECT 
	category.name,
	EXISTS(
    	SELECT 1
	    FROM item_category
	    WHERE category.id = item_category.cat_id
        	AND item_category.item_id = ?1
	) AS "selected"
FROM category;