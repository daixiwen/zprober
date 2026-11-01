library ieee;
use ieee.std_logic_1164.all;
use ieee.numeric_std.all;
use ieee.math_real.all;

entity a_submodule is
    port (
        clk   : in std_logic;
        reset : in std_logic;
        
        c     : in std_logic;
        d     : out std_logic
    );
end entity a_submodule;

architecture rtl of a_submodule is

begin

    process (clk)
    begin
        if rising_edge(clk) then
            if reset = '1' then
                d <= '0';
            else
                d <= c;
            end if;
        end if;
    end process;

end architecture;
