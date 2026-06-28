CREATE TABLE employee_rates (
    id          BIGSERIAL    PRIMARY KEY,
    employee_id BIGINT       NOT NULL REFERENCES employees(id) ON DELETE CASCADE,
    label       TEXT         NOT NULL,
    amount      NUMERIC(10,2) NOT NULL,
    created_at  TIMESTAMPTZ  NOT NULL DEFAULT NOW()
);