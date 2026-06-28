CREATE TABLE profiles (
    id           BIGSERIAL    PRIMARY KEY,
    employee_id  BIGINT       NOT NULL REFERENCES employees(id) ON DELETE CASCADE,
    label        TEXT         NOT NULL,
    class_name   TEXT         NOT NULL,
    teacher_room TEXT         NOT NULL,
    details      TEXT         NOT NULL,
    created_at   TIMESTAMPTZ  NOT NULL DEFAULT NOW()
);