library ieee;
use ieee.std_logic_1164.all;
use ieee.numeric_std.all;
use ieee.math_real.all;

entity top is
    port (
        rec   : out t_rec;
        arr   : out t_arr;
        state : out t_state
    );

    type t_state is (idle, running, done, 'x');
    type t_rec is record
        a : std_logic;
        b : t_state;
    end record;
    type t_arr is array (0 to 7) of t_rec;
end entity top;
