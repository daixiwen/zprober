library ieee;
use ieee.std_logic_1164.all;
use ieee.numeric_std.all;
use ieee.math_real.all;

entity my_module is
    port (
        clk   : in std_logic;
        reset : in std_logic;
        
        d : in std_logic;
        b : out std_logic
    );
end entity my_module;

architecture rtl of my_module is

begin

    process (clk)
    begin
        if rising_edge(clk) then
            if reset = '1' then
                b <= '0';
            else
                b <= d;
            end if;
        end if;
    end process;

end architecture;