library ieee;
use ieee.std_logic_1164.all;
use ieee.numeric_std.all;

entity top is
    port (
        bit_in    : in std_logic;
        vector_in : in std_logic_vector(7 downto 0);
        count     : in integer range 0 to 15;
        num       : in unsigned(15 downto 0);
        flag      : in boolean
    );
end entity top;