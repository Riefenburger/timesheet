-- Each row is one logged chunk of work by one person.

CREATE TABLE time_entries (
    id          BIGSERIAL    PRIMARY KEY,
    user_name   TEXT         NOT NULL,
    entry_date  DATE         NOT NULL,
    hours       NUMERIC(5,2) NOT NULL,
    description TEXT         NOT NULL,
    created_at  TIMESTAMPTZ  NOT NULL DEFAULT NOW()
);