-- Add migration script here
CREATE TABLE IF NOT EXISTS challenge (
  id serial PRIMARY KEY,
	username VARCHAR (255) NOT NULL,
	time_limit INT,
	opponent_time_limit INT,
	increment INT,
	color VARCHAR (10),
	sats BIGINT,
	opp_username VARCHAR (255) NOT NULL,
	status VARCHAR (255),
	lichess_challenge_id VARCHAR (255),
	result VARCHAR (255),
	created_on TIMESTAMP without time zone default (now() at time zone 'utc'),
	expire_after INT,
	payment_addr VARCHAR (255),
  payment_request VARCHAR (510),
  opp_payment_addr VARCHAR (255),
  opp_payment_request VARCHAR (510)
);

CREATE INDEX IF NOT EXISTS challenge_username_idx ON challenge(username);
CREATE INDEX IF NOT EXISTS challenge_opp_username_idx ON challenge(opp_username);