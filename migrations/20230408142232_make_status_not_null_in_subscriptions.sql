-- Wrapping this migration in a transaction
BEGIN;
    -- Backfill `status` column
    UPDATE subscriptions
        SET status = 'confirmed'
        WHERE status IS NULL;
    -- Add NOT NULL constraint
    ALTER TABLE subscriptions
        ALTER COLUMN status SET NOT NULL;
COMMIT;
