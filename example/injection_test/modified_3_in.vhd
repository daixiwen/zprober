library ieee;
use ieee.std_logic_1164.all;
use ieee.numeric_std.all;
use ieee.math_real.all;

entity original is
    port (
        clk   : in std_logic;
        reset : in std_logic;
        
        data_in : in std_logic_vector(7 downto 0);
        data_out: out std_logic_vector(7 downto 0)
-- Zprober[[   the user has added some content here
;
    new_debug : out std_logic_vector(15 downto 0)
-- ZProber]]
    );
end entity original;

architecture rtl of original is

    signal data_internal : std_logic_vector(7 downto 0);

-- Zprober[[  the next comment will also have some extra spaces
    signal new_extra_signal : std_logic_vector(15 downto 0);
-- ZProber]]       
begin

    i_component_0 : entity work.my_component
        port map(
            clk => clk,
            reset => reset,
            data_in => data_in,
            data_out => data_internal
        );

    i_component_1 : entity work.my_other_component
        generic map (
            bugs => false
        )
        port map(
            clk => clk,
            reset => reset,
            data_in => data_internal,
            data_out => data_out
-- Zprober[[
,
    new_debug => new_debug
-- ZProber]]
        );

end architecture;
