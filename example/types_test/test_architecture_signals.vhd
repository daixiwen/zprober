library ieee;
use ieee.std_logic_1164.all;
use ieee.numeric_std.all;
use ieee.math_real.all;


entity top is
    port (p : out t_shared);

    type t_shared is array (0 to 1) of std_logic;
end entity top;

architecture rtl of top is

    type t_local is record
        f : t_shared;
    end record;

    signal s1 : t_local;
    signal s2 : t_shared;

begin
end architecture rtl;
