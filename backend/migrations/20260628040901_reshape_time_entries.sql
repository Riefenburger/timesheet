-- Drop the old demo version. Its single curl-test row goes with it;
-- it has no employee to attach to under the new shape.
DROP TABLE time_entries;

-- Recreate it as the digital time card, linked to a real employee.
CREATE TABLE time_entries (
    id           BIGSERIAL    PRIMARY KEY,
    employee_id  BIGINT       NOT NULL REFERENCES employees(id) ON DELETE RESTRICT,
    entry_date   DATE         NOT NULL,
    class_name   TEXT         NOT NULL,
    teacher_room TEXT         NOT NULL,
    details      TEXT         NOT NULL,
    hours        NUMERIC(5,2) NOT NULL,
    created_at   TIMESTAMPTZ  NOT NULL DEFAULT NOW()
);