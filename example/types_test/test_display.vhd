library ieee;
use ieee.std_logic_1164.all;
use ieee.numeric_std.all;
use ieee.math_real.all;


entity top is
    port (
        a : out std_logic_vector(3 downto 0);
        b : out t_state
    );

    type t_state is (low, high);
end entity top;
